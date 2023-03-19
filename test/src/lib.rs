use {
    anyhow::{bail, Result},
    wast::{
        core::{Custom, CustomPlace, Export, Import, ModuleField, ModuleKind},
        parser::{self, ParseBuffer},
        token::Span,
        Wat,
    },
};

pub fn retarget_imports(target: &str, module: &[u8]) -> Result<(Vec<u8>, Vec<String>)> {
    let wat = wasmprinter::print_bytes(&module)?;
    let buffer = ParseBuffer::new(&wat)?;
    let wat = parser::parse::<Wat>(&buffer)?;
    let mut module = match wat {
        Wat::Module(module) => module,
        Wat::Component(_) => bail!("expected module; got component"),
    };

    let fields = match &mut module.kind {
        ModuleKind::Text(fields) => fields,
        ModuleKind::Binary(_) => unreachable!(),
    };

    let mut exports = Vec::new();
    let mut names = Vec::new();

    for (index, field) in fields.iter().enumerate() {
        match field {
            ModuleField::Import(Import { module, field, .. }) => {
                if *module != target {
                    names.push((index, format!("{module}:{field}")));
                }
            }

            ModuleField::Export(Export { name, .. }) => exports.push((*name).to_owned()),

            _ => (),
        }
    }

    for (index, name) in &names {
        if let ModuleField::Import(Import { module, field, .. }) = &mut fields[*index] {
            assert!(*module != target);
            *module = target;
            *field = name;
        } else {
            unreachable!()
        }
    }

    let result = module.encode()?;

    std::fs::write("/tmp/bar.wasm", &result)?;

    Ok((result, exports))
}

pub fn add_custom_section(name: &str, value: Vec<u8>, module: &[u8]) -> Result<Vec<u8>> {
    let wat = wasmprinter::print_bytes(&module)?;
    let buffer = ParseBuffer::new(&wat)?;
    let wat = parser::parse::<Wat>(&buffer)?;
    let mut module = match wat {
        Wat::Module(module) => module,
        Wat::Component(_) => bail!("expected module; got component"),
    };

    let fields = match &mut module.kind {
        ModuleKind::Text(fields) => fields,
        ModuleKind::Binary(_) => unreachable!(),
    };

    fields.push(ModuleField::Custom(Custom {
        span: Span::from_offset(0),
        name,
        place: CustomPlace::AfterLast,
        data: vec![&value],
    }));

    let result = module.encode()?;

    std::fs::write("/tmp/baz.wasm", &result)?;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use {
        anyhow::{anyhow, Result},
        spin_abi_conformance::{
            InvocationStyle, KeyValueReport, MysqlReport, PostgresReport, RedisReport, Report,
            TestConfig, WasiReport,
        },
        std::{collections::HashSet, path::PathBuf},
        tokio::fs,
        wasmtime::{component::Component, Config, Engine},
        wit_component::ComponentEncoder,
        wit_parser::Resolve,
    };

    include!(concat!(env!("OUT_DIR"), "/wasms.rs"));

    static EXPORT_INTERFACES: &[(&str, &str)] = &[
        ("handle-redis-message", "inbound-redis"),
        ("handle-http-request", "inbound-http"),
    ];

    async fn run(module: &[u8]) -> Result<()> {
        let (module, exports) = crate::retarget_imports("wasi_snapshot_preview1", module)?;

        let mut resolve = Resolve::default();
        let (pkg, _) =
            resolve.push_dir(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../adapter/wit"))?;

        let world = resolve.select_world(pkg, Some("reactor"))?;

        let allowed = exports
            .into_iter()
            .filter_map(|export| {
                EXPORT_INTERFACES
                    .iter()
                    .find_map(|(k, v)| (*k == &export).then_some(*v))
            })
            .collect::<HashSet<&str>>();

        resolve
            .worlds
            .get_mut(world)
            .unwrap()
            .exports
            .retain(|k, _| allowed.contains(&k.as_str()));

        let type_ = wit_component::metadata::encode(
            &resolve,
            world,
            wit_component::StringEncoding::UTF8,
            None,
        )?;

        let adapter =
            crate::add_custom_section("component-type:reactor", type_, &fs::read(ADAPTER).await?)?;

        let component = ComponentEncoder::default()
            .validate(true)
            .module(&module)?
            .adapter("wasi_snapshot_preview1", &adapter)?
            .encode()?;

        let mut config = Config::new();
        config.wasm_component_model(true);
        config.async_support(true);

        let engine = Engine::new(&config)?;

        let component = Component::new(&engine, component)?;

        let report = spin_abi_conformance::test(
            &component,
            &engine,
            TestConfig {
                invocation_style: InvocationStyle::InboundHttp,
            },
        )
        .await?;

        let expected = Report {
            inbound_http: Ok(()),
            inbound_redis: Ok(()),
            config: Ok(()),
            http: Ok(()),
            redis: RedisReport {
                publish: Ok(()),
                set: Ok(()),
                get: Ok(()),
                incr: Ok(()),
                del: Ok(()),
                sadd: Ok(()),
                srem: Ok(()),
                smembers: Ok(()),
                execute: Ok(()),
            },
            postgres: PostgresReport {
                execute: Ok(()),
                query: Ok(()),
            },
            mysql: MysqlReport {
                execute: Ok(()),
                query: Ok(()),
            },
            key_value: KeyValueReport {
                open: Ok(()),
                get: Ok(()),
                set: Ok(()),
                delete: Ok(()),
                exists: Ok(()),
                get_keys: Ok(()),
                close: Ok(()),
            },
            wasi: WasiReport {
                env: Ok(()),
                epoch: Ok(()),
                random: Ok(()),
                stdio: Ok(()),
                read: Ok(()),
                readdir: Ok(()),
                stat: Ok(()),
            },
        };

        if report == expected {
            Ok(())
        } else {
            Err(anyhow!("{report:#?}"))
        }
    }

    #[tokio::test]
    async fn rust() -> Result<()> {
        run(&fs::read(RUST_CASE).await?).await
    }

    #[tokio::test]
    async fn go() -> Result<()> {
        run(&fs::read(GO_CASE).await?).await
    }
}

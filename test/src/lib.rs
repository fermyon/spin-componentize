use {
    anyhow::{bail, Result},
    wast::{
        core::{Import, ModuleField, ModuleKind},
        parser::{self, ParseBuffer},
        Wat,
    },
};

pub fn retarget_imports(target: &str, module: &[u8]) -> Result<Vec<u8>> {
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

    let mut names = Vec::new();

    for field in &*fields {
        if let ModuleField::Import(Import { module, field, .. }) = field {
            if *module != target {
                names.push(format!("{module}:{field}"));
            }
        }
    }

    let mut names = names.iter();

    for field in fields {
        if let ModuleField::Import(Import { module, field, .. }) = field {
            if *module != target {
                *module = target;
                *field = names.next().unwrap();
            }
        }
    }

    let result = module.encode()?;

    std::fs::write("/tmp/bar.wasm", &result)?;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use {
        crate::retarget_imports,
        anyhow::{anyhow, Result},
        spin_abi_conformance::{
            InvocationStyle, KeyValueReport, MysqlReport, PostgresReport, RedisReport, Report,
            TestConfig, WasiReport,
        },
        tokio::fs,
        wasmtime::{component::Component, Config, Engine},
        wit_component::ComponentEncoder,
    };

    include!(concat!(env!("OUT_DIR"), "/wasms.rs"));

    #[tokio::test]
    async fn it_works() -> Result<()> {
        let component = ComponentEncoder::default()
            .validate(true)
            .module(&retarget_imports(
                "wasi_snapshot_preview1",
                &fs::read(TEST_CASE).await?,
            )?)?
            .adapter("wasi_snapshot_preview1", &fs::read(ADAPTER).await?)?
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
}

#![deny(warnings)]

use {
    anyhow::{anyhow, Result},
    convert::{IntoEntityType, IntoExportKind},
    std::{borrow::Cow, collections::HashSet},
    wasm_encoder::{CustomSection, ExportSection, ImportSection, Module, RawSection},
    wasmparser::{Encoding, Parser, Payload},
    wit_component::{metadata, ComponentEncoder},
};

mod convert;

const SPIN_ADAPTER: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/wasm32-unknown-unknown/release/wasi_snapshot_preview1_spin.wasm"
));

const COMMAND_ADAPTER: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/wasm32-unknown-unknown/release/wasi_snapshot_preview1_command.wasm"
));

static ADAPTER_NAME: &str = "wasi_snapshot_preview1";
static CUSTOM_SECTION_NAME: &str = "component-type:reactor";
static WORLD_NAME: &str = "reactor";

static EXPORT_INTERFACES: &[(&str, &str)] = &[
    ("handle-redis-message", "inbound-redis"),
    ("handle-http-request", "inbound-http"),
];

pub fn componentize_if_necessary(module_or_component: &[u8]) -> Result<Cow<[u8]>> {
    for payload in Parser::new(0).parse_all(module_or_component) {
        match payload? {
            Payload::Version { encoding, .. } => {
                return match encoding {
                    Encoding::Component => Ok(Cow::Borrowed(module_or_component)),
                    Encoding::Module => componentize(module_or_component).map(Cow::Owned),
                }
            }
            _ => (),
        }
    }

    Err(anyhow!("unable to determine Wasm encoding"))
}

pub fn componentize(module: &[u8]) -> Result<Vec<u8>> {
    let (module, exports) = retarget_imports_and_get_exports(ADAPTER_NAME, module)?;

    let (adapter, mut bindgen) = metadata::decode(SPIN_ADAPTER)?;

    let allowed = exports
        .into_iter()
        .filter_map(|export| {
            EXPORT_INTERFACES
                .iter()
                .find_map(|(k, v)| (*k == &export).then_some(*v))
        })
        .collect::<HashSet<&str>>();

    let world = bindgen
        .resolve
        .worlds
        .iter()
        .find_map(|(k, v)| (v.name == WORLD_NAME).then_some(k))
        .ok_or_else(|| anyhow!("world not found: {WORLD_NAME}"))?;

    bindgen.resolve.worlds[world]
        .exports
        .retain(|k, _| allowed.contains(&k.as_str()));

    let body = metadata::encode(
        &bindgen.resolve,
        world,
        wit_component::StringEncoding::UTF8,
        None,
    )?;

    let adapter = add_custom_section(CUSTOM_SECTION_NAME, &body, &adapter)?;

    Ok(ComponentEncoder::default()
        .validate(true)
        .module(&module)?
        .adapter(ADAPTER_NAME, &adapter)?
        .encode()?)
}

pub fn componentize_command(module: &[u8]) -> Result<Vec<u8>> {
    Ok(ComponentEncoder::default()
        .validate(true)
        .module(&module)?
        .adapter(ADAPTER_NAME, COMMAND_ADAPTER)?
        .encode()?)
}

fn retarget_imports_and_get_exports(target: &str, module: &[u8]) -> Result<(Vec<u8>, Vec<String>)> {
    let mut result = Module::new();
    let mut exports_result = Vec::new();

    for payload in Parser::new(0).parse_all(module) {
        match payload? {
            Payload::ImportSection(reader) => {
                let mut imports = ImportSection::new();
                for import in reader {
                    let import = import?;
                    let (module, field) = if import.module == target {
                        (Cow::Borrowed(import.module), Cow::Borrowed(import.name))
                    } else {
                        (
                            Cow::Borrowed(target),
                            Cow::Owned(format!("{}:{}", import.module, import.name)),
                        )
                    };
                    imports.import(&module, &field, IntoEntityType(import.ty));
                }
                result.section(&imports);
            }

            Payload::ExportSection(reader) => {
                let mut exports = ExportSection::new();
                for export in reader {
                    let export = export?;
                    exports_result.push(export.name.to_owned());
                    exports.export(
                        export.name,
                        IntoExportKind(export.kind).into(),
                        export.index,
                    );
                }
                result.section(&exports);
            }

            payload => {
                if let Some((id, range)) = payload.as_section() {
                    result.section(&RawSection {
                        id,
                        data: &module[range],
                    });
                }
            }
        }
    }

    Ok((result.finish(), exports_result))
}

fn add_custom_section(name: &str, data: &[u8], module: &[u8]) -> Result<Vec<u8>> {
    let mut result = Module::new();

    for payload in Parser::new(0).parse_all(module) {
        if let Some((id, range)) = payload?.as_section() {
            result.section(&RawSection {
                id,
                data: &module[range],
            });
        }
    }

    result.section(&CustomSection { name, data });

    Ok(result.finish())
}

#[cfg(test)]
mod tests {
    use {
        anyhow::{anyhow, Result},
        host::{
            self,
            command::wasi::{Command, InputStream, OutputStream},
            WasiCtx,
        },
        spin_abi_conformance::{
            InvocationStyle, KeyValueReport, MysqlReport, PostgresReport, RedisReport, Report,
            TestConfig, WasiReport,
        },
        std::io::Cursor,
        tokio::fs,
        wasi_cap_std_sync::WasiCtxBuilder,
        wasi_common::pipe::{ReadPipe, WritePipe},
        wasmtime::{
            component::{Component, Linker},
            Config, Engine, Store,
        },
    };

    async fn run_spin(module: &[u8]) -> Result<()> {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.async_support(true);

        let engine = Engine::new(&config)?;

        let component = Component::new(&engine, crate::componentize(module)?)?;

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

    async fn run_command(module: &[u8]) -> Result<()> {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.async_support(true);

        let engine = Engine::new(&config)?;

        let mut linker = Linker::<WasiCtx>::new(&engine);
        host::command::add_to_linker(&mut linker, |context| context)?;

        let mut store = Store::new(&engine, WasiCtxBuilder::new().build());

        let component = Component::new(&engine, crate::componentize_command(module)?)?;

        let (wasi, _) = Command::instantiate_async(&mut store, &component, &linker).await?;

        store
            .data_mut()
            .set_stdin(Box::new(ReadPipe::new(Cursor::new(
                "So rested he by the Tumtum tree",
            ))));

        let stdout = WritePipe::new_in_memory();
        store.data_mut().set_stdout(Box::new(stdout.clone()));

        wasi.call_main(
            &mut store,
            0 as InputStream,
            1 as OutputStream,
            2 as OutputStream,
            &["Jabberwocky"],
            &[],
        )
        .await?
        .map_err(|()| anyhow!("command returned with failing exit status"))?;

        drop(store);

        let stdout = stdout.try_into_inner().unwrap().into_inner();

        assert_eq!(
            b"Jabberwocky\nSo rested he by the Tumtum tree" as &[_],
            &stdout
        );

        Ok(())
    }

    #[tokio::test]
    async fn rust() -> Result<()> {
        run_spin(
            &fs::read(concat!(
                env!("OUT_DIR"),
                "/wasm32-wasi/release/rust_case.wasm"
            ))
            .await?,
        )
        .await
    }

    #[tokio::test]
    async fn go() -> Result<()> {
        run_spin(&fs::read(concat!(env!("OUT_DIR"), "/go_case.wasm")).await?).await
    }

    #[tokio::test]
    async fn rust_command() -> Result<()> {
        run_command(
            &fs::read(concat!(
                env!("OUT_DIR"),
                "/wasm32-wasi/release/rust-command.wasm"
            ))
            .await?,
        )
        .await
    }
}

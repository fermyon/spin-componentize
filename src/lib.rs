#![deny(warnings)]

use {
    anyhow::{anyhow, Context, Result},
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
const PREVIEW1_ADAPTER: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/wasm32-unknown-unknown/release/wasi_snapshot_preview1_upstream.wasm"
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
        match payload.context("unable to parse binary")? {
            Payload::Version { encoding, .. } => {
                return match encoding {
                    Encoding::Component => Ok(Cow::Borrowed(module_or_component)),
                    Encoding::Module => componentize(module_or_component).map(Cow::Owned),
                };
            }
            _ => (),
        }
    }
    Err(anyhow!("unable to determine wasm binary encoding"))
}

pub fn componentize(module: &[u8]) -> Result<Vec<u8>> {
    match WitBindgenVersion::from_module(module)? {
        WitBindgenVersion::V0_2 => componentize_old_bindgen(module),
        WitBindgenVersion::V0_8 => componentize_new_bindgen(module),
        WitBindgenVersion::Other(other) => Err(anyhow::anyhow!(
            "cannot adapt modules created with wit-bindgen version {other}"
        )),
    }
}

/// In order to properly componentize modules, we need to know which
/// version of wit-bindgen was used
#[derive(Debug)]
enum WitBindgenVersion {
    V0_8,
    V0_2,
    Other(String),
}

impl WitBindgenVersion {
    fn from_module(module: &[u8]) -> Result<Self> {
        let (_, bindgen) = metadata::decode(module)?;
        if let Some(producers) = bindgen.producers {
            if let Some(processors) = producers.get("processed-by") {
                let bindgen_version = processors.iter().find_map(|(key, value)| {
                    key.starts_with("wit-bindgen").then(|| value.as_str())
                });
                match bindgen_version {
                    Some(v) if v.starts_with("0.8.") => return Ok(Self::V0_8),
                    Some(other) => return Ok(Self::Other(other.to_owned())),
                    None => {}
                }
            }
        }

        Ok(Self::V0_2)
    }
}

/// Modules produced with wit-bindgen 0.5 and newer only need wasi preview 1 to preview 2 adapter
pub fn componentize_new_bindgen(module: &[u8]) -> Result<Vec<u8>> {
    ComponentEncoder::default()
        .validate(true)
        .module(&module)?
        .adapter("wasi_snapshot_preview1", PREVIEW1_ADAPTER)?
        .encode()
}

/// Modules produced with wit-bindgen 0.2 need more extensive adaption
pub fn componentize_old_bindgen(module: &[u8]) -> Result<Vec<u8>> {
    let (module, exports) = retarget_imports_and_get_exports(ADAPTER_NAME, module)?;
    let allowed = exports
        .into_iter()
        .filter_map(|export| {
            EXPORT_INTERFACES
                .iter()
                .find_map(|(k, v)| (*k == &export).then_some(*v))
        })
        .collect::<HashSet<&str>>();

    let (adapter, mut bindgen) = metadata::decode(SPIN_ADAPTER)?;

    let world = bindgen
        .resolve
        .worlds
        .iter()
        .find_map(|(k, v)| (v.name == WORLD_NAME).then_some(k))
        .ok_or_else(|| anyhow!("world not found: {WORLD_NAME}"))?;

    bindgen.resolve.worlds[world].exports.retain(|k, _| {
        let k = match &k {
            wit_parser::WorldKey::Name(n) => n,
            wit_parser::WorldKey::Interface(i) => match &bindgen.resolve.interfaces[*i].name {
                Some(n) => n,
                None => return true,
            },
        };
        allowed.contains(k.as_str())
    });

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

    result.section(&CustomSection {
        name: Cow::Borrowed(name),
        data: Cow::Borrowed(data),
    });

    Ok(result.finish())
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, process};

    use anyhow::Context;
    use wasmtime_wasi::preview2::pipe::MemoryOutputPipe;

    use {
        anyhow::{anyhow, Result},
        spin_abi_conformance::{
            InvocationStyle, KeyValueReport, LlmReport, MysqlReport, PostgresReport, RedisReport,
            Report, TestConfig, WasiReport,
        },
        tokio::fs,
        wasmtime::{
            component::{Component, Linker},
            Config, Engine, Store,
        },
        wasmtime_wasi::preview2::{
            command::Command, pipe::MemoryInputPipe, IsATTY, Table, WasiView,
        },
        wasmtime_wasi::preview2::{WasiCtx, WasiCtxBuilder},
    };

    async fn run_spin(module: &[u8]) -> Result<()> {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.async_support(true);

        let engine = Engine::new(&config)?;

        let component = Component::new(
            &engine,
            crate::componentize(module).context("could not componentize")?,
        )
        .context("failed to instantiate componentized bytes")?;

        let report = spin_abi_conformance::test(
            &component,
            &engine,
            TestConfig {
                invocation_style: InvocationStyle::InboundHttp,
            },
        )
        .await
        .context("abi conformance test failed")?;

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
            llm: LlmReport { infer: Ok(()) },
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

        struct Wasi {
            ctx: WasiCtx,
            table: Table,
        }
        impl WasiView for Wasi {
            fn table(&self) -> &Table {
                &self.table
            }

            fn table_mut(&mut self) -> &mut Table {
                &mut self.table
            }

            fn ctx(&self) -> &WasiCtx {
                &self.ctx
            }

            fn ctx_mut(&mut self) -> &mut WasiCtx {
                &mut self.ctx
            }
        }

        let mut linker = Linker::<Wasi>::new(&engine);
        wasmtime_wasi::preview2::command::add_to_linker(&mut linker)?;
        let mut ctx = WasiCtxBuilder::new();
        let stdout = MemoryOutputPipe::new(1024);
        ctx.stdin(
            MemoryInputPipe::new("So rested he by the Tumtum tree".into()),
            IsATTY::Yes,
        )
        .stdout(stdout.clone(), IsATTY::Yes)
        .args(&["Jabberwocky"]);

        let mut table = Table::new();
        let wasi = Wasi {
            ctx: ctx.build(&mut table).unwrap(),
            table,
        };

        let mut store = Store::new(&engine, wasi);

        let component = Component::new(&engine, crate::componentize_command(module)?)?;

        let (wasi, _) = Command::instantiate_async(&mut store, &component, &linker).await?;

        wasi.wasi_cli_run()
            .call_run(&mut store)
            .await?
            .map_err(|()| anyhow!("command returned with failing exit status"))?;

        drop(store);

        let stdout = stdout.try_into_inner().unwrap().to_vec();

        assert_eq!(
            b"Jabberwocky\nSo rested he by the Tumtum tree" as &[_],
            &stdout
        );

        Ok(())
    }

    #[tokio::test]
    async fn rust_wit_bindgen_02() -> Result<()> {
        build_rust_test_case("rust-case-0.2");
        run_spin(
            &fs::read(concat!(
                env!("OUT_DIR"),
                "/wasm32-wasi/release/rust_case_02.wasm"
            ))
            .await?,
        )
        .await
    }

    #[tokio::test]
    async fn rust_wit_bindgen_08() -> Result<()> {
        build_rust_test_case("rust-case-0.8");
        run_spin(
            &fs::read(concat!(
                env!("OUT_DIR"),
                "/wasm32-wasi/release/rust_case_08.wasm"
            ))
            .await?,
        )
        .await
    }

    #[ignore]
    #[tokio::test]
    async fn go() -> Result<()> {
        let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        let mut cmd = process::Command::new("tinygo");
        cmd.arg("build")
            .current_dir("tests/go-case")
            .arg("-target=wasi")
            .arg("-gc=leaking")
            .arg("-no-debug")
            .arg("-o")
            .arg(out_dir.join("go_case.wasm"))
            .arg("main.go");

        // If just skip this if TinyGo is not installed
        _ = cmd.status();
        run_spin(&fs::read(concat!(env!("OUT_DIR"), "/go_case.wasm")).await?).await
    }

    #[tokio::test]
    async fn rust_command() -> Result<()> {
        build_rust_test_case("rust-command");
        run_command(
            &fs::read(concat!(
                env!("OUT_DIR"),
                "/wasm32-wasi/release/rust-command.wasm"
            ))
            .await?,
        )
        .await
    }

    fn build_rust_test_case(name: &str) {
        let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        let mut cmd = process::Command::new("cargo");
        cmd.arg("build")
            .current_dir(&format!("tests/{name}"))
            .arg("--release")
            .arg("--target=wasm32-wasi")
            .env("CARGO_TARGET_DIR", out_dir);

        let status = cmd.status().unwrap();
        assert!(status.success());
    }
}

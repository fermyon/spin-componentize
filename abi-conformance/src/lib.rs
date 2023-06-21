//! Spin ABI Conformance Test Suite
//!
//! This crate provides a suite of tests to check a given SDK or language integration's implementation of Spin
//! functions.  It is intended for use by language integrators and SDK authors to verify that their integrations
//! and SDKs work correctly with the Spin ABIs.  It is not intended for Spin _application_ development, since it
//! requires a component written specifically to behave as expected by this suite, whereas a given application will
//! have its own expected behaviors which can only be verified by tests specific to that application.
//!
//! The suite may be run via the [`test()`] function, which accepts a [`wasmtime::component::Component`] and a
//! [`TestConfig`] and returns a [`Report`] which details which tests succeeded and which failed.  The definition
//! of success in this context depends on whether the test is for a function implemented by the guest
//! (i.e. inbound requests) or by the host (i.e. outbound requests).
//!
//! - For a guest-implemented function, the host will call the function and assert the result matches what is
//! expected (see [`Report::inbound_http`] for an example).
//!
//! - For a host-implemented function, the host will call a guest-implemented function according to the specified
//! [`InvocationStyle`] with a set of arguments indicating which host function to call and with what arguments.
//! The host then asserts that host function was indeed called with the expected arguments (see
//! [`Report::http`] for an example).

#![deny(warnings)]

use anyhow::{anyhow, bail, Context as _, Result};
use fermyon::spin::http_types::{Method, Request, Response};
use serde::{Deserialize, Serialize};
use std::{future::Future, str};
use test_config::Config;
use test_http::Http;
use test_key_value::KeyValue;
use test_mysql::Mysql;
use test_postgres::Postgres;
use test_redis::Redis;
use wasmtime::{
    component::{Component, InstancePre, Linker},
    Engine, Store,
};
use wasmtime_wasi::preview2::{
    pipe::WritePipe, stream::TableStreamExt, OutputStream, Table, WasiCtx, WasiCtxBuilder, WasiView,
};

pub use test_key_value::KeyValueReport;
pub use test_mysql::MysqlReport;
pub use test_postgres::PostgresReport;
pub use test_redis::RedisReport;
pub use test_wasi::WasiReport;

mod test_config;
mod test_http;
mod test_inbound_http;
mod test_inbound_redis;
mod test_key_value;
mod test_mysql;
mod test_postgres;
mod test_redis;
mod test_wasi;

wasmtime::component::bindgen!({
    path: "../wasmtime/crates/wasi/wit",
    world: "fermyon:spin/reactor",
    async: true
});
pub use fermyon::spin::*;

/// The invocation style to use when the host asks the guest to call a host-implemented function
#[derive(Copy, Clone, Default, Deserialize)]
pub enum InvocationStyle {
    /// The host should call into the guest using inbound-http.wit's `handle-request` function, passing arguments
    /// via the request body as a string of tokens separated by the delimiter "%20".
    #[default]
    InboundHttp,
}

/// Configuration options for the [`test()`] function
#[derive(Default, Deserialize)]
pub struct TestConfig {
    /// The invocation style to use when the host asks the guest to call a host-implemented function
    #[serde(default)]
    pub invocation_style: InvocationStyle,
}

/// Report of which tests succeeded or failed
///
/// These results fall into either of two categories:
///
/// - Guest-implemented exports which behave as prescribed by the test (e.g. `inbound_http` and `inbound_redis`)
///
/// - Host-implemented imports which are called by the guest with the arguments specified by the host
/// (e.g. `http`)
#[derive(Serialize, PartialEq, Eq, Debug)]
pub struct Report {
    /// Result of the Spin inbound HTTP test
    ///
    /// The guest component should expect a call to `handle-request` with a POST request to "/foo" containing
    /// a single header "foo: bar" and a UTF-8 string body "Hello, SpinHttp!" and return a 200 OK response that
    /// includes a single header "lorem: ipsum" and a UTF-8 string body "dolor sit amet".
    pub inbound_http: Result<(), String>,

    /// Result of the Spin inbound Redis test
    ///
    /// The guest component should expect a call to `handle-message` with the text "Hello, SpinRedis!" and return
    /// `ok(unit)` as the result.
    pub inbound_redis: Result<(), String>,

    /// Result of the Spin config test
    ///
    /// The guest component should expect a call according to [`InvocationStyle`] with \["config", "foo"\] as
    /// arguments.  The component should call the host-implemented `config::get-config` function with "foo" as the
    /// argument and expect `ok("bar")` as the result.  The host will assert that said function is called exactly
    /// once with the expected argument.
    pub config: Result<(), String>,

    /// Result of the Spin outbound HTTP test
    ///
    /// The guest component should expect a call according to [`InvocationStyle`] with \["http",
    /// "http://127.0.0.1/test"\] as arguments.  The component should call the host-implemented
    /// `http::send-request` function with a GET request for the URL "http://127.0.0.1/test" with no headers,
    /// params, or body, and expect `ok({ status: 200, headers: none, body: some("Jabberwocky"))` as the result.
    /// The host will assert that said function is called exactly once with the specified argument.
    pub http: Result<(), String>,

    /// Results of the Spin Redis tests
    ///
    /// See [`RedisReport`] for details.
    pub redis: RedisReport,

    /// Results of the Spin PostgreSQL tests
    ///
    /// See [`PostgresReport`] for details.
    pub postgres: PostgresReport,

    /// Results of the Spin MySql tests
    ///
    /// See [`MysqlReport`] for details.
    pub mysql: MysqlReport,

    /// Results of the Spin key-value tests
    ///
    /// See [`KeyValueReport`] for details.
    pub key_value: KeyValueReport,

    /// Results of the WASI tests
    ///
    /// See [`WasiReport`] for details.
    pub wasi: WasiReport,
}

/// Run a test for each Spin-related function the specified `component` imports or exports, returning the results
/// as a [`Report`].
///
/// See the fields of [`Report`] and the structs from which it is composed for descriptions of each test.
pub async fn test(
    component: &Component,
    engine: &Engine,
    test_config: TestConfig,
) -> Result<Report> {
    let mut table = Table::new();
    let mut store = Store::new(
        engine,
        Context {
            test_config,
            wasi: WasiCtxBuilder::new()
                .build(&mut table)
                .expect("building wasi context failed"),
            table,
            http: Http::default(),
            redis: Redis::default(),
            postgres: Postgres::default(),
            mysql: Mysql::default(),
            key_value: KeyValue::default(),
            config: Config::default(),
        },
    );

    let mut linker = Linker::<Context>::new(engine);
    wasmtime_wasi::preview2::wasi::command::add_to_linker(&mut linker)?;
    http::add_to_linker(&mut linker, |context| &mut context.http)?;
    redis::add_to_linker(&mut linker, |context| &mut context.redis)?;
    postgres::add_to_linker(&mut linker, |context| &mut context.postgres)?;
    mysql::add_to_linker(&mut linker, |context| &mut context.mysql)?;
    key_value::add_to_linker(&mut linker, |context| &mut context.key_value)?;
    config::add_to_linker(&mut linker, |context| &mut context.config)?;

    let pre = linker.instantiate_pre(component)?;

    Ok(Report {
        inbound_http: test_inbound_http::test(&mut store, &pre).await,
        inbound_redis: test_inbound_redis::test(&mut store, &pre).await,
        config: test_config::test(&mut store, &pre).await,
        http: test_http::test(&mut store, &pre).await,
        redis: test_redis::test(&mut store, &pre).await?,
        postgres: test_postgres::test(&mut store, &pre).await?,
        mysql: test_mysql::test(&mut store, &pre).await?,
        key_value: test_key_value::test(&mut store, &pre).await?,
        wasi: test_wasi::test(&mut store, &pre).await?,
    })
}

struct Context {
    test_config: TestConfig,
    wasi: WasiCtx,
    table: Table,
    http: Http,
    redis: Redis,
    postgres: Postgres,
    mysql: Mysql,
    key_value: KeyValue,
    config: Config,
}

impl WasiView for Context {
    fn table(&self) -> &Table {
        &self.table
    }

    fn table_mut(&mut self) -> &mut Table {
        &mut self.table
    }

    fn ctx(&self) -> &WasiCtx {
        &self.wasi
    }

    fn ctx_mut(&mut self) -> &mut WasiCtx {
        &mut self.wasi
    }
}

async fn run(fun: impl Future<Output = Result<()>>) -> Result<(), String> {
    fun.await.map_err(|e| format!("{e:?}"))
}

async fn run_command(
    store: &mut Store<Context>,
    pre: &InstancePre<Context>,
    arguments: &[&str],
    fun: impl FnOnce(&mut Store<Context>) -> Result<()>,
) -> Result<(), String> {
    run(async {
        let stderr = WritePipe::new_in_memory();
        set_stderr(store, &stderr);

        let instance = pre.instantiate_async(&mut *store).await?;

        match store.data().test_config.invocation_style {
            InvocationStyle::InboundHttp => {
                let func = instance
                    .exports(&mut *store)
                    .instance("fermyon:spin/inbound-http")
                    .ok_or_else(|| anyhow!("no fermyon:spin/inbound-http instance found"))?
                    .typed_func::<(Request,), (Response,)>("handle-request")?;

                let result = func
                    .call_async(
                        &mut *store,
                        (Request {
                            method: Method::Post,
                            uri: "/".into(),
                            headers: vec![],
                            params: vec![],
                            body: Some(arguments.join("%20").into_bytes()),
                        },),
                    )
                    .await;

                // Reset `Context::wasi` and `Context::table` so the next test has a clean slate and also to ensure there are no more
                // references to the `stderr` pipe, ensuring `try_into_inner` succeeds below.  This is also needed
                // in case the caller attached its own pipes for e.g. stdin and/or stdout and expects exclusive
                // ownership once we return.
                let mut table = Table::new();
                store.data_mut().wasi = WasiCtxBuilder::new()
                    .build(&mut table)
                    .expect("failed to reset wasi context");
                *store.data_mut().table_mut() = table;

                let (response,) = result.with_context(|| {
                    String::from_utf8_lossy(&stderr.try_into_inner().unwrap().into_inner())
                        .into_owned()
                })?;

                if response.status != 200 {
                    bail!(
                        "status: {}; body: {}",
                        response.status,
                        response
                            .body
                            .as_deref()
                            .map(|body| String::from_utf8_lossy(body))
                            .unwrap_or_default()
                    );
                }
            }
        }

        fun(store)
    })
    .await
}

fn set_stderr(store: &mut Store<Context>, stderr: &WritePipe<std::io::Cursor<Vec<u8>>>) {
    let stderr_key = store.data().wasi.stderr;
    store
        .data_mut()
        .table_mut()
        .delete::<Box<dyn OutputStream>>(stderr_key)
        .unwrap();
    store.data_mut().wasi.stderr = store
        .data_mut()
        .table_mut()
        .push_output_stream(Box::new(stderr.clone()))
        .unwrap();
}

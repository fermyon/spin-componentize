#[cfg(test)]
mod tests {
    use {
        anyhow::Result,
        http_types::{HttpError, Method, RequestParam, RequestResult, Response},
        tokio::fs,
        wasi_cap_std_sync::WasiCtxBuilder,
        wasi_common::WasiCtx,
        wasmtime::{
            component::{Component, Linker},
            Config, Engine, Store,
        },
        wit_component::ComponentEncoder,
    };

    include!(concat!(env!("OUT_DIR"), "/wasms.rs"));

    wasmtime::component::bindgen!({
        path: "../adapter/wit",
        world: "reactor",
        async: true
    });

    #[derive(Default)]
    struct Host;

    #[async_trait::async_trait]
    impl outbound_http::Host for Host {
        async fn send_request(
            &mut self,
            request: RequestResult,
        ) -> Result<Result<Response, HttpError>> {
            Ok(
                if request.method == Method::Post
                    && "/hello" == &request.uri
                    && &[("foo".to_owned(), "bar".to_owned())] as &[_] == &request.headers
                    && Some(b"hola" as &[_]) == request.body.as_deref()
                {
                    Ok(Response {
                        status: 200,
                        headers: Some(vec![("foo".to_owned(), "bar".to_owned())]),
                        body: Some(b"hello, world!".to_vec()),
                    })
                } else {
                    Err(HttpError::RequestError)
                },
            )
        }
    }

    struct State {
        wasi: WasiCtx,
        host: Host,
    }

    #[tokio::test]
    async fn it_works() -> Result<()> {
        let component = ComponentEncoder::default()
            .validate(true)
            .module(&fs::read(TEST_CASE).await?)?
            .adapter("wasi_snapshot_preview1", &fs::read(ADAPTER).await?)?
            .alias("wasi-outbound-http", "wasi_snapshot_preview1")
            .encode()?;

        let mut config = Config::new();
        config.wasm_component_model(true);
        config.async_support(true);

        let engine = Engine::new(&config)?;

        let mut linker = Linker::new(&engine);
        outbound_http::add_to_linker(&mut linker, |state: &mut State| &mut state.host)?;
        host::add_to_linker(&mut linker, |state: &mut State| &mut state.wasi)?;

        let mut store = Store::new(
            &engine,
            State {
                wasi: WasiCtxBuilder::new().build(),
                host: Host::default(),
            },
        );

        let component = Component::new(&engine, component)?;

        let (reactor, _) = Reactor::instantiate_async(&mut store, &component, &linker).await?;

        let response = reactor
            .inbound_http
            .call_handle_request(
                &mut store,
                RequestParam {
                    method: Method::Post,
                    uri: "/hello",
                    headers: &[("foo", "bar")],
                    params: &[],
                    body: Some(b"hola"),
                },
            )
            .await?;

        assert_eq!(response.status, 200);
        assert_eq!(
            response.headers,
            Some(vec![("foo".to_owned(), "bar".to_owned())])
        );
        assert_eq!(response.body, Some(b"hello, world!".to_vec()));

        Ok(())
    }
}

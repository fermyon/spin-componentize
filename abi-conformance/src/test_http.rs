use crate::{
    http,
    http_types::{HttpError, RequestResult, Response},
    Context,
};
use anyhow::{ensure, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use wasmtime::{component::InstancePre, Store};

#[derive(Default)]
pub(crate) struct Http {
    map: HashMap<String, String>,
}

#[async_trait]
impl http::Host for Http {
    async fn send_request(&mut self, req: RequestResult) -> Result<Result<Response, HttpError>> {
        Ok(self
            .map
            .remove(&req.uri)
            .map(|body| Response {
                status: 200,
                headers: None,
                body: Some(body.into_bytes()),
            })
            .ok_or(HttpError::InvalidUrl))
    }
}

pub(crate) async fn test(
    store: &mut Store<Context>,
    pre: &InstancePre<Context>,
) -> Result<(), String> {
    store
        .data_mut()
        .http
        .map
        .insert("http://127.0.0.1/test".into(), "Jabberwocky".into());

    crate::run_command(store, pre, &["http", "http://127.0.0.1/test"], |store| {
        ensure!(
            store.data().http.map.is_empty(),
            "expected module to call `wasi-outbound-http::request` exactly once"
        );

        Ok(())
    })
    .await
}

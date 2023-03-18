use crate::{
    http_types::{Method, RequestParam},
    Context, Reactor,
};
use anyhow::ensure;
use wasmtime::{component::InstancePre, Store};

pub(crate) async fn test(
    store: &mut Store<Context>,
    pre: &InstancePre<Context>,
) -> Result<(), String> {
    crate::run(async {
        let (reactor, _) = Reactor::instantiate_pre(&mut *store, pre).await?;
        let response = reactor
            .inbound_http
            .call_handle_request(
                store,
                RequestParam {
                    method: Method::Post,
                    uri: "/foo",
                    headers: &[("foo", "bar")],
                    params: &[],
                    body: Some(b"Hello, SpinHttp!"),
                },
            )
            .await?;

        ensure!(
            response.status == 200,
            "expected response status 200, got {} (body: {:?})",
            response.status,
            response
                .body
                .as_ref()
                .map(|body| String::from_utf8_lossy(body))
        );

        ensure!(
            response
                .headers
                .as_ref()
                .map(|v| v.len() == 1 && "lorem" == &v[0].0.to_lowercase() && "ipsum" == &v[0].1)
                .unwrap_or(false),
            "expected a single response header, \"lorem: ipsum\", got {:?}",
            response.headers
        );

        let expected_body = "dolor sit amet";

        ensure!(
            response.body == Some(expected_body.as_bytes().to_vec()),
            "expected a response body containing the string {expected_body:?}, got {:?}",
            response
                .body
                .as_ref()
                .map(|body| String::from_utf8_lossy(body))
        );

        Ok(())
    })
    .await
}

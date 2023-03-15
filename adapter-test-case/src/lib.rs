use anyhow::{Error, Result};
use spin_sdk::{
    http::{Request, Response},
    http_component,
};

#[http_component]
fn send_outbound(req: Request) -> Result<Response> {
    spin_sdk::outbound_http::send_request(req).map_err(Error::from)
}

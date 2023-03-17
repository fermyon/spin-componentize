use crate::{redis_types, Context, Reactor};
use std::{error, fmt};
use wasmtime::{component::InstancePre, Store};

impl fmt::Display for redis_types::Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Success => f.write_str("redis success"),
            Self::Error => f.write_str("redis error"),
        }
    }
}

impl error::Error for redis_types::Error {}

pub(crate) async fn test(
    store: &mut Store<Context>,
    pre: &InstancePre<Context>,
) -> Result<(), String> {
    super::run(async {
        let (reactor, _) = Reactor::instantiate_pre(&mut *store, pre).await?;
        reactor
            .inbound_redis
            .call_handle_message(store, b"Hello, SpinRedis!")
            .await??;

        Ok(())
    })
    .await
}

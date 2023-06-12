use crate::{
    redis_types::{Error, Payload},
    Context,
};
use anyhow::anyhow;
use std::{error, fmt};
use wasmtime::{component::InstancePre, Store};

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Success => f.write_str("redis success"),
            Self::Error => f.write_str("redis error"),
        }
    }
}

impl error::Error for Error {}

pub(crate) async fn test(
    store: &mut Store<Context>,
    pre: &InstancePre<Context>,
) -> Result<(), String> {
    super::run(async {
        let instance = pre.instantiate_async(&mut *store).await?;

        let func = instance
            .exports(&mut *store)
            .instance("inbound-redis")
            .ok_or_else(|| anyhow!("no inbound-redis instance found"))?
            .typed_func::<(Payload,), (Result<(), Error>,)>("handle-message")?;

        match func
            .call_async(store, (b"Hello, SpinRedis!".to_vec(),))
            .await?
        {
            (Ok(()) | Err(Error::Success),) => Ok(()),
            (Err(e),) => Err(e.into()),
        }
    })
    .await
}

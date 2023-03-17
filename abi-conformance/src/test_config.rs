use crate::{config, Context};
use anyhow::{ensure, Result};
use std::collections::HashMap;
use wasmtime::{
    component::{InstancePre, __internal::async_trait},
    Store,
};

#[derive(Default)]
pub(super) struct Config {
    map: HashMap<String, String>,
}

#[async_trait]
impl config::Host for Config {
    async fn get_config(&mut self, key: String) -> Result<Result<String, config::Error>> {
        Ok(self
            .map
            .remove(&key)
            .ok_or_else(|| config::Error::InvalidKey(key.to_owned())))
    }
}

pub(crate) async fn test(
    store: &mut Store<Context>,
    pre: &InstancePre<Context>,
) -> Result<(), String> {
    store
        .data_mut()
        .config
        .map
        .insert("foo".into(), "bar".into());

    crate::run_command(store, pre, &["config", "foo"], |store| {
        ensure!(
            store.data().config.map.is_empty(),
            "expected module to call `spin-config::get-config` exactly once"
        );

        Ok(())
    })
    .await
}

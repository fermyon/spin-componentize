# spin-componentize

This library converts a Spin module to a
[component](https://github.com/WebAssembly/component-model/).

See [reactor.wit](wasmtime/crates/wasi/wit/deps/preview/reactor.wit) for the definition of the Spin world.
Note that although the world specifies both `inbound-redis` and `inbound-http`
exports, `spin-componentize` will only export either or both according to what
the original module exported.

## Building

First, install [Rust](https://rustup.rs/) v1.68 or later.  You'll also need to
install a couple of Wasm targets:

```shell
rustup target add wasm32-wasi
rustup target add wasm32-unknown-unknown
```

Then run `cargo build --release`.  Note that this is currently only a library
and does not yet have a CLI interface, although that would be easy to add if
desired.

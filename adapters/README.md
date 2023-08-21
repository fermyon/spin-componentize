# Adapters

The componentize process uses adapters to adapt plain wasm modules to wasi preview 2 compatible wasm components. There are three adapters that are built and stored as wasm binaries in this repository:

* The upstream wasi preview1 adapters for both commands and reactors for use with newer versions of wit-bindgen (v0.5 and above).
    * These are currently built using the commit [592ddc52](https://github.com/bytecodealliance/wasmtime/commit/592ddc5209ce1d76832c0ff1c81393507db8cef4).  You can rebuild using `./ci/build-wasi-preview1-component-adapter.sh` and copy them into this repo using e.g. `cp target/wasm32-unknown-unknown/release/wasi_snapshot_preview1.*.wasm ../spin-componentize/adapters/`
* A modified adapter that has knowledge of Spin APIs for use with v0.2 of wit-bindgen which has a different ABI than newer wit-bindgen based modules.
    * This is currently built using commit [9d85e13f](https://github.com/dicej/wasmtime/commit/9d85e13f51a02c84756ba2b161d35979896a113d) on the github.com/dicej/wasmtime fork of wasmtime.  You can rebuild it using `cargo build -p wasi-preview1-component-adapter --target wasm32-unknown-unknown --release` and copy it into this repo using e.g. `cp target/wasm32-unknown-unknown/release/wasi_snapshot_preview1.wasm ../spin-componentize/adapters/wasi_snapshot_preview1.spin.wasm`.

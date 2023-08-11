# Adapters

The componentize process uses adapters to adapt plain wasm modules to wasi preview 2 compatible wasm components. There are three adapters that are built and stored as wasm binaries in this repository:

* The upstream wasi preview1 adapters for both commands and reactors for use with newer versions of wit-bindgen (v0.5 and above).
    * These are currently the [v10.0.1 release](https://github.com/bytecodealliance/wasmtime/releases/tag/v10.0.1).
* A modified adapter that has knowledge of Spin APIs for use with v0.2 of wit-bindgen which has a different ABI than newer wit-bindgen based modules.
    * This is currently built using commit [8e261ac4](https://github.com/rylev/wasmtime/commit/8e261ac452ff54031efe2fde804cdf63fded3e55) on the github.com/rylev/wasmtime fork of wasmtime.

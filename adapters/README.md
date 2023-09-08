# Adapters

The componentize process uses adapters to adapt plain wasm modules to wasi preview 2 compatible wasm components. There are three adapters that are built and stored as wasm binaries in this repository:

* The upstream wasi preview1 adapters for both commands and reactors for use with newer versions of wit-bindgen (v0.5 and above).
    * These are currently the [v10.0.1 release](https://github.com/bytecodealliance/wasmtime/releases/tag/v10.0.1).
* A modified adapter that has knowledge of Spin APIs for use with v0.2 of wit-bindgen which has a different ABI than newer wit-bindgen based modules.
    * This is currently built using commit [9555713](https://github.com/rylev/wasmtime/commit/955571392155e428a5f8be585c9a569f1f0b94c7) on the github.com/rylev/wasmtime fork of wasmtime.
    * Changes are usually done on the `spin-adapter` branch of that fork.


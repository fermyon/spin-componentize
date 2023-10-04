# Adapters

The componentize process uses adapters to adapt plain wasm modules to wasi preview 2 compatible wasm components. There are three adapters that are built and stored as wasm binaries in this repository:

* The upstream wasi preview1 adapters for both commands and reactors for use with newer versions of wit-bindgen (v0.5 and above).
    * These are currently built using commit [d4e4f610](https://github.com/bytecodealliance/wasmtime/commit/d4e4f610ce86289619e5962ae13031fec9e5d71d)
* A modified adapter that has knowledge of Spin APIs for use with v0.2 of wit-bindgen which has a different ABI than newer wit-bindgen based modules.
    * This is currently built using commit [155f88c9](https://github.com/dicej/wasmtime/commit/155f88c98e09f5d598fd5c3ad0a0594c9b8f652e) on the github.com/dicej/wasmtime fork of wasmtime.



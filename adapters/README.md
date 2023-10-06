# Adapters

The componentize process uses adapters to adapt plain wasm modules to wasi preview 2 compatible wasm components. There are three adapters that are built and stored as wasm binaries in this repository:

* The upstream wasi preview1 adapters for both commands and reactors for use with newer versions of wit-bindgen (v0.5 and above).
    * These are currently built using commit [05731177](https://github.com/bytecodealliance/wasmtime/commit/0573117736a698a6a03715ea41bcd1a1a7b9fa4d)
* A modified adapter that has knowledge of Spin APIs for use with v0.2 of wit-bindgen which has a different ABI than newer wit-bindgen based modules.
    * This is currently built using commit [abbe04a2](https://github.com/dicej/wasmtime/commit/abbe04a28757c645db57f66db2bc9a80e9ce8148) on the github.com/dicej/wasmtime fork of wasmtime.



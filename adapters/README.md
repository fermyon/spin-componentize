# Adapters

The componentize process uses adapters to adapt plain wasm modules to wasi preview 2 compatible wasm components. There are three adapters that are built and stored as wasm binaries in this repository:

* The upstream wasi preview1 adapters for both commands and reactors for use with newer versions of wit-bindgen (v0.5 and above).
    * These are currently a [commit on main](https://github.com/bytecodealliance/wasmtime/commit/4c34504efb258a0c51c6a5f3f8a5b24d987993b9) as we await the wasmtime 14 release.
* A modified adapter that has knowledge of Spin APIs for use with v0.2 of wit-bindgen which has a different ABI than newer wit-bindgen based modules.
    * This is currently built using commit [484350](https://github.com/rylev/wasmtime/commit/48435059e0916294cc6870ffd5bcb649de3b82b2) on the github.com/rylev/wasmtime fork of wasmtime.
    * You can see a diff between the upstream wasmtime 14 compatible adapter and this custom adapter [here](https://github.com/bytecodealliance/wasmtime/compare/4c34504efb258a0c51c6a5f3f8a5b24d987993b9...rylev:wasmtime:spin-adapter-wasmtime14).


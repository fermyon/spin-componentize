# Adapters

The componentize process uses adapters to adapt plain wasm modules to wasi preview 2 compatible wasm components. There are three adapters that are built and stored as wasm binaries in this repository:

* The upstream wasi preview1 adapters for both commands and reactors for use with newer versions of wit-bindgen (v0.5 and above).
    * These are currently a [commit on the wastime 14.0.0 release branch](https://github.com/bytecodealliance/wasmtime/commit/2ffbc36c377b98e4eabe89ae37bb334605d904cc) as we await the wasmtime 14 release.
* A modified adapter that has knowledge of Spin APIs for use with v0.2 of wit-bindgen which has a different ABI than newer wit-bindgen based modules.
    * This is currently built using commit [484350](https://github.com/rylev/wasmtime/commit/350e10e54e724c004d938bfd6f90d7e6ba1a3518) on the github.com/rylev/wasmtime fork of wasmtime.
    * You can see a diff between the upstream wasmtime 14 compatible adapter and this custom adapter [here](https://github.com/bytecodealliance/wasmtime/compare/2ffbc36c377b98e4eabe89ae37bb334605d904cc...rylev:wasmtime:350e10e54e724c004d938bfd6f90d7e6ba1a3518).


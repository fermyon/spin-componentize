# Tests

The various sample applications ensure that all various flavors of Spin like binaries can run against the same wasmtime based runtime. In particular, `rust-case-02` and `rust-case-07` test that binaries built using wit-bindgen 0.2 and 0.7 respectively behave the same when run through `spin_componentize`.
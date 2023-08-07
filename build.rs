use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

/// The git sha of the wasmtime commit the adapter WebAssembly modules
/// found in './adapters' have been built from.
const ADAPTERS_SHA: &str = "c4eafed";

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let adapters_dir = Path::new("adapters").join(ADAPTERS_SHA);

    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .current_dir("wasmtime/crates/wasi-preview1-component-adapter")
        .arg("--release")
        .arg("--target=wasm32-unknown-unknown")
        .env("CARGO_TARGET_DIR", &out_dir);

    let status = cmd.status().unwrap();
    assert!(status.success());
    println!("cargo:rerun-if-changed=wasmtime/crates/wasi-preview1-component-adapter");
    fs::copy(
        out_dir.join("wasm32-unknown-unknown/release/wasi_snapshot_preview1.wasm"),
        out_dir.join("wasm32-unknown-unknown/release/wasi_snapshot_preview1_spin.wasm"),
    )
    .unwrap();

    fs::copy(
        adapters_dir.join("wasi_snapshot_preview1.reactor.wasm"),
        out_dir.join("wasm32-unknown-unknown/release/wasi_snapshot_preview1_upstream.wasm"),
    )
    .unwrap();

    fs::copy(
        adapters_dir.join("wasi_snapshot_preview1.command.wasm"),
        out_dir.join("wasm32-unknown-unknown/release/wasi_snapshot_preview1_command.wasm"),
    )
    .unwrap();

    build_rust_test_case(&out_dir, "rust-case-0.2");
    build_rust_test_case(&out_dir, "rust-case-0.8");
    build_rust_test_case(&out_dir, "rust-command");

    let mut cmd = Command::new("tinygo");
    cmd.arg("build")
        .current_dir("tests/go-case")
        .arg("-target=wasi")
        .arg("-gc=leaking")
        .arg("-no-debug")
        .arg("-o")
        .arg(out_dir.join("go_case.wasm"))
        .arg("main.go");

    // If just skip this if TinyGo is not installed
    _ = cmd.status();
    println!("cargo:rerun-if-changed=go-case");
}

fn build_rust_test_case(out_dir: &PathBuf, name: &str) {
    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .current_dir(&format!("tests/{name}"))
        .arg("--release")
        .arg("--target=wasm32-wasi")
        .env("CARGO_TARGET_DIR", out_dir);

    let status = cmd.status().unwrap();
    assert!(status.success());
    println!("cargo:rerun-if-changed=tests/{name}");
}

use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .current_dir("../adapter")
        .arg("--release")
        .arg("--target=wasm32-unknown-unknown")
        .env("CARGO_TARGET_DIR", &out_dir);

    let status = cmd.status().unwrap();
    assert!(status.success());
    println!("cargo:rerun-if-changed=../adapter");
    let adapter = out_dir.join("wasm32-unknown-unknown/release/wasi_snapshot_preview1.wasm");

    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .current_dir("../rust-case")
        .arg("--release")
        .arg("--target=wasm32-wasi")
        .env("CARGO_TARGET_DIR", &out_dir);

    let status = cmd.status().unwrap();
    assert!(status.success());
    println!("cargo:rerun-if-changed=../rust-case");
    let rust_case = out_dir.join("wasm32-wasi/release/rust_case.wasm");

    let mut cmd = Command::new("tinygo");
    cmd.arg("build")
        .current_dir("../go-case")
        .arg("-target=wasi")
        .arg("-gc=leaking")
        .arg("-no-debug")
        .arg("-o")
        .arg(out_dir.join("go_case.wasm"))
        .arg("main.go");

    let status = cmd.status().unwrap();
    assert!(status.success());
    println!("cargo:rerun-if-changed=../go-case");
    let go_case = out_dir.join("go_case.wasm");

    let src = format!(
        "
            const ADAPTER: &str = {adapter:?};
            const RUST_CASE: &str = {rust_case:?};
            const GO_CASE: &str = {go_case:?};
        ",
    );

    fs::write(out_dir.join("wasms.rs"), src).unwrap();
}

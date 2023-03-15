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
        .current_dir("../adapter-test-case")
        .arg("--release")
        .arg("--target=wasm32-wasi")
        .env("CARGO_TARGET_DIR", &out_dir);

    let status = cmd.status().unwrap();
    assert!(status.success());
    println!("cargo:rerun-if-changed=../adapter-test-case");
    let test_case = out_dir.join("wasm32-wasi/release/adapter_test_case.wasm");

    let src = format!(
        "
            pub const ADAPTER: &str = {adapter:?};
            pub const TEST_CASE: &str = {test_case:?};
        ",
    );

    fs::write(out_dir.join("wasms.rs"), src).unwrap();
}

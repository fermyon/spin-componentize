use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .current_dir("adapter")
        .arg("--release")
        .arg("--target=wasm32-unknown-unknown")
        .env("CARGO_TARGET_DIR", &out_dir);

    let status = cmd.status().unwrap();
    assert!(status.success());
    println!("cargo:rerun-if-changed=adapter");
    fs::rename(
        out_dir.join("wasm32-unknown-unknown/release/wasi_snapshot_preview1.wasm"),
        out_dir.join("wasm32-unknown-unknown/release/wasi_snapshot_preview1_spin.wasm"),
    )
    .unwrap();

    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .current_dir("adapter")
        .arg("--release")
        .arg("--no-default-features")
        .arg("--features")
        .arg("command")
        .arg("--target=wasm32-unknown-unknown")
        .env("CARGO_TARGET_DIR", &out_dir);

    let status = cmd.status().unwrap();
    assert!(status.success());
    println!("cargo:rerun-if-changed=adapter");
    fs::rename(
        out_dir.join("wasm32-unknown-unknown/release/wasi_snapshot_preview1.wasm"),
        out_dir.join("wasm32-unknown-unknown/release/wasi_snapshot_preview1_command.wasm"),
    )
    .unwrap();

    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .current_dir("rust-case")
        .arg("--release")
        .arg("--target=wasm32-wasi")
        .env("CARGO_TARGET_DIR", &out_dir);

    let status = cmd.status().unwrap();
    assert!(status.success());
    println!("cargo:rerun-if-changed=rust-case");

    let mut cmd = Command::new("tinygo");
    cmd.arg("build")
        .current_dir("go-case")
        .arg("-target=wasi")
        .arg("-gc=leaking")
        .arg("-no-debug")
        .arg("-o")
        .arg(out_dir.join("go_case.wasm"))
        .arg("main.go");

    // If just skip this if TinyGo is not installed
    _ = cmd.status();
    println!("cargo:rerun-if-changed=go-case");

    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .current_dir("rust-command")
        .arg("--release")
        .arg("--target=wasm32-wasi")
        .env("CARGO_TARGET_DIR", &out_dir);

    let status = cmd.status().unwrap();
    assert!(status.success());
    println!("cargo:rerun-if-changed=rust-command");
}

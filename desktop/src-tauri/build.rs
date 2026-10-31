//! The app trusts exactly one certificate authority: the owner's home CA. Its public certificate is
//! copied into the binary at build time from the file named by `PERO_CA_FILE` (default
//! `~/.pero-ca/ca.crt`), so it is never read from the repository or from the system's trust store.

use std::{env, fs, path::PathBuf};

fn main() {
    let source = env::var_os("PERO_CA_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env::var_os("HOME").expect("HOME is not set")).join(".pero-ca/ca.crt")
        });
    println!("cargo:rerun-if-env-changed=PERO_CA_FILE");
    println!("cargo:rerun-if-changed={}", source.display());
    let pem = fs::read_to_string(&source).unwrap_or_else(|e| {
        panic!(
            "cannot read the CA certificate {} ({e}); set PERO_CA_FILE to the home CA's ca.crt",
            source.display()
        )
    });
    assert!(
        pem.contains("-----BEGIN CERTIFICATE-----") && !pem.contains("PRIVATE KEY"),
        "{} must be the public CA certificate (and nothing secret)",
        source.display()
    );
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("ca.crt"),
        pem,
    )
    .unwrap();
    tauri_build::build();
}

use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=SI_RELEASE_PUBLIC_KEY_PATH");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("publisher_key.rs");
    let key = env::var_os("SI_RELEASE_PUBLIC_KEY_PATH")
        .map(fs::read)
        .transpose()
        .expect("failed to read SI_RELEASE_PUBLIC_KEY_PATH");
    if let Some(key) = &key {
        assert_eq!(key.len(), 32, "Ed25519 public key must be 32 raw bytes");
    }
    let value = key
        .map(|bytes| format!("&{:?}", bytes))
        .unwrap_or_else(|| "&[]".into());
    fs::write(out, value).expect("failed to generate embedded publisher key");
}

use sha2::{Digest, Sha256};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path};

pub mod config;
#[cfg(windows)]
pub mod credentials;
pub mod nas;
pub mod provision;
pub mod validate;

pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Artifact {
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NasConfig {
    pub host: String,
    pub manifest_path: String,
}

impl NasConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.host != "https://file.satreci.com" { return Err("NAS host must be https://file.satreci.com".into()); }
        safe_name(self.manifest_path.strip_prefix('/').ok_or("Manifest path must start with /")?)
    }
}

pub fn safe_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.contains(['\\', ':']) || name.split('/').any(|part| part.is_empty() || part == "." || part == "..")
        || !Path::new(name).components().all(|part| matches!(part, Component::Normal(_))) {
        return Err(format!("Unsafe path: {name}"));
    }
    Ok(())
}

pub fn hash_file(path: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 65536];
    loop { let count = file.read(&mut buffer)?; if count == 0 { break; } hasher.update(&buffer[..count]); }
    Ok(format!("{:x}", hasher.finalize()))
}

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Component, Path};

pub mod config;
#[cfg(windows)]
pub mod credentials;
pub mod nas;
pub mod provision;
pub mod release_download;
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
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_manifest_path")]
    pub manifest_path: String,
    #[serde(default = "default_release_path")]
    pub release_path: String,
}

pub fn default_release_path() -> String {
    "/volume1/SI\u{c870}\u{c9c1}_\u{c6b0}\u{c8fc}\u{ae30}\u{c220}\u{c13c}\u{d130}/\u{b0d0}\u{acf5}\u{c720}_\u{c6b0}\u{c8fc}\u{ae30}\u{c220}\u{ac1c}\u{bc1c}\u{c790}\u{b8cc}/Software Tools/si_opencode/current/release.json".into()
}

fn default_host() -> String {
    "https://file.satreci.com".into()
}
fn default_manifest_path() -> String {
    "/volume1/SI\u{c870}\u{c9c1}_\u{c6b0}\u{c8fc}\u{ae30}\u{c220}\u{c13c}\u{d130}/\u{b0d0}\u{acf5}\u{c720}_\u{c6b0}\u{c8fc}\u{ae30}\u{c220}\u{ac1c}\u{bc1c}\u{c790}\u{b8cc}/Software Tools/si_opencode/extensions-manifest.json".into()
}

impl Default for NasConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            manifest_path: default_manifest_path(),
            release_path: default_release_path(),
        }
    }
}

impl NasConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.host != "https://file.satreci.com" {
            return Err("NAS host must be https://file.satreci.com".into());
        }
        safe_name(
            self.manifest_path
                .strip_prefix('/')
                .ok_or("Manifest path must start with /")?,
        )?;
        safe_name(
            self.release_path
                .strip_prefix('/')
                .ok_or("Release path must start with /")?,
        )
    }
}

pub fn safe_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.contains(['\\', ':'])
        || name
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || !Path::new(name)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err(format!("Unsafe path: {name}"));
    }
    Ok(())
}

pub fn hash_file(path: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_path_defaults_and_can_be_overridden_from_local_config() {
        let default: NasConfig = serde_json::from_str(
            r#"{"host":"https://file.satreci.com","manifest_path":"/extensions.json"}"#,
        )
        .unwrap();
        assert_eq!(default.release_path, default_release_path());
        let configured: NasConfig = serde_json::from_str(r#"{"host":"https://file.satreci.com","manifest_path":"/extensions.json","release_path":"/custom/current/release.json"}"#).unwrap();
        assert_eq!(configured.release_path, "/custom/current/release.json");
        assert!(configured.validate().is_ok());
        let sample: NasConfig =
            serde_json::from_str(include_str!("../config/extensions.example.json")).unwrap();
        assert_eq!(sample.release_path, default_release_path());
    }
}

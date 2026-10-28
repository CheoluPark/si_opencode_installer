use crate::{
    nas::{Nas, NasError},
    safe_name, Artifact, NasConfig,
};
use ring::signature::{UnparsedPublicKey, ED25519};
use serde::Deserialize;
use std::{collections::BTreeMap, fs, path::Path};

const PUBLISHER_KEY: &[u8] = include!(concat!(env!("OUT_DIR"), "/publisher_key.rs"));
const MANIFEST_LIMIT: u64 = 4 * 1024 * 1024;
const SIGNATURE_LIMIT: u64 = 128;

#[derive(Deserialize)]
pub struct ReleaseManifest {
    pub version: String,
    pub files: BTreeMap<String, Artifact>,
}

pub fn download_current(
    nas: &Nas,
    config: &NasConfig,
    staging: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir(staging)?;
    let result = download_and_verify(nas, config, staging, PUBLISHER_KEY);
    if result.is_err() {
        let _ = fs::remove_dir_all(staging);
    }
    result
}

fn download_and_verify(
    nas: &Nas,
    config: &NasConfig,
    staging: &Path,
    public_key: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let signature_path = config
        .release_path
        .strip_suffix("release.json")
        .ok_or("Release path must end in release.json")?
        .to_owned()
        + "release.sig";
    let manifest_file = staging.join("release.json");
    let signature_file = staging.join("release.sig");
    nas.download(&config.release_path, &manifest_file, MANIFEST_LIMIT, None)?;
    nas.download(&signature_path, &signature_file, SIGNATURE_LIMIT, None)?;
    let manifest_bytes = fs::read(&manifest_file)?;
    let signature = fs::read(&signature_file)?;
    let manifest = verify_manifest(&manifest_bytes, &signature, public_key)?;
    for (name, artifact) in &manifest.files {
        safe_name(name)?;
        if !(name.starts_with(".config/opencode/") || name.starts_with("tools/")) {
            return Err(format!("Release file is outside allowed roots: {name}").into());
        }
        if artifact.bytes > 2 * 1024 * 1024 * 1024 {
            return Err(format!("Release file exceeds size limit: {name}").into());
        }
        let target = staging.join(name);
        fs::create_dir_all(target.parent().ok_or("Invalid release path")?)?;
        let remote = format!(
            "{}/{}",
            config
                .release_path
                .rsplit_once('/')
                .ok_or("Invalid release path")?
                .0,
            name
        );
        nas.download(&remote, &target, artifact.bytes, Some(artifact))?;
    }
    if !manifest.files.contains_key("tools/tools-manifest.json")
        || !manifest
            .files
            .keys()
            .any(|name| name.starts_with(".config/opencode/"))
        || !manifest.files.keys().any(|name| name.starts_with("tools/"))
    {
        return Err("Release is missing required config or tools content".into());
    }
    Ok(())
}

pub fn verify_manifest(
    bytes: &[u8],
    signature: &[u8],
    public_key: &[u8],
) -> Result<ReleaseManifest, Box<dyn std::error::Error>> {
    if public_key.len() != 32 {
        return Err("Publisher verification key is not configured".into());
    }
    UnparsedPublicKey::new(&ED25519, public_key)
        .verify(bytes, signature)
        .map_err(|_| "Invalid release signature")?;
    let manifest: ReleaseManifest = serde_json::from_slice(bytes)?;
    if manifest.version.trim().is_empty() || manifest.files.is_empty() {
        return Err("Invalid empty release manifest".into());
    }
    Ok(manifest)
}

#[cfg(windows)]
pub fn current_version_from_local_config() -> Result<String, Box<dyn std::error::Error>> {
    use crate::credentials;
    let config_path = std::path::PathBuf::from(std::env::var("LOCALAPPDATA")?)
        .join("SI OpenCode/release.json");
    let config = if config_path.is_file() { serde_json::from_slice(&fs::read(config_path)?)? } else { NasConfig::default() };
    let mut account = match credentials::load()? {
        Some(account) => account,
        None => credentials::prompt()?.ok_or("NAS sign-in cancelled")?,
    };
    loop {
        match Nas::login(&config, &account.0, &account.1) {
            Ok(nas) => {
                credentials::save(&account.0, &account.1)?;
                let temp = tempfile::tempdir()?;
                let manifest = temp.path().join("release.json");
                let signature = temp.path().join("release.sig");
                let signature_path = config.release_path.strip_suffix("release.json").ok_or("Release path must end in release.json")?.to_owned() + "release.sig";
                let result = (|| -> Result<String, Box<dyn std::error::Error>> {
                    nas.download(&config.release_path, &manifest, MANIFEST_LIMIT, None)?;
                    nas.download(&signature_path, &signature, SIGNATURE_LIMIT, None)?;
                    Ok(verify_manifest(&fs::read(&manifest)?, &fs::read(&signature)?, PUBLISHER_KEY)?.version)
                })();
                let _ = nas.logout();
                return result;
            }
            Err(NasError::Authentication) => account = credentials::prompt()?.ok_or("NAS sign-in cancelled")?,
            Err(error) => return Err(error.into()),
        }
    }
}

#[cfg(windows)]
pub fn fetch_for_installer(
    staging: &Path,
    config: &NasConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::credentials;
    let mut account = match credentials::load()? {
        Some(account) => account,
        None => credentials::prompt()?.ok_or("NAS sign-in cancelled")?,
    };
    loop {
        match Nas::login(config, &account.0, &account.1) {
            Ok(nas) => {
                credentials::save(&account.0, &account.1)?;
                let result = download_current(&nas, config, staging);
                let _ = nas.logout();
                return result;
            }
            Err(NasError::Authentication) => {
                account = credentials::prompt()?.ok_or("NAS sign-in cancelled")?
            }
            Err(error) => return Err(error.into()),
        }
    }
}

#[cfg(windows)]
pub fn fetch_from_local_config(staging: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::PathBuf::from(std::env::var("LOCALAPPDATA")?)
        .join("SI OpenCode/extensions.json");
    let config = if path.is_file() {
        serde_json::from_slice(&fs::read(path)?)?
    } else {
        NasConfig::default()
    };
    fetch_for_installer(staging, &config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::{
        rand::SystemRandom,
        signature::{Ed25519KeyPair, KeyPair},
    };

    #[test]
    fn verifies_exact_signed_manifest_and_rejects_tampering() {
        let key = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).unwrap();
        let signer = Ed25519KeyPair::from_pkcs8(key.as_ref()).unwrap();
        let bytes =
            br#"{"version":"1","files":{"tools/tools-manifest.json":{"sha256":"00","bytes":1}}}"#;
        let signature = signer.sign(bytes);
        assert!(verify_manifest(bytes, signature.as_ref(), signer.public_key().as_ref()).is_ok());
        let changed =
            br#"{"version":"2","files":{"tools/tools-manifest.json":{"sha256":"00","bytes":1}}}"#;
        assert!(
            verify_manifest(changed, signature.as_ref(), signer.public_key().as_ref()).is_err()
        );
    }

    #[test]
    fn rejects_empty_or_untrusted_manifests() {
        assert!(verify_manifest(b"{}", b"", &[0; 32]).is_err());
        let key = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).unwrap();
        let signer = Ed25519KeyPair::from_pkcs8(key.as_ref()).unwrap();
        let bytes = br#"{"version":"","files":{}}"#;
        assert!(verify_manifest(
            bytes,
            signer.sign(bytes).as_ref(),
            signer.public_key().as_ref()
        )
        .is_err());
    }
}

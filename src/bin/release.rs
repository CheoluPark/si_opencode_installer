use ring::{
    rand::SystemRandom,
    signature::{Ed25519KeyPair, KeyPair},
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    env, fs, io,
    path::{Path, PathBuf},
};

#[derive(Serialize)]
struct Artifact {
    sha256: String,
    bytes: u64,
}

#[derive(Serialize)]
struct Manifest {
    version: String,
    files: BTreeMap<String, Artifact>,
    schema_hashes: BTreeMap<String, String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("keygen") => {
            let private = PathBuf::from(args.next().ok_or("Expected private-key.pk8")?);
            let public = PathBuf::from(args.next().ok_or("Expected public-key.pub")?);
            let pkcs8 = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).map_err(|_| "Key generation failed")?;
            let pair = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).map_err(|_| "Generated key is invalid")?;
            fs::write(private, pkcs8.as_ref())?;
            fs::write(public, pair.public_key().as_ref())?;
        }
        Some("build") => {
            let root = PathBuf::from(args.next().ok_or("Expected staging directory")?);
            let version = args.next().ok_or("Expected release version")?;
            let key_path = PathBuf::from(args.next().ok_or("Expected private key")?);
            build(&root, &version, &key_path)?;
        }
        _ => return Err("Usage: release keygen <private-key.pk8> <public-key.pub> | build <staging-dir> <version> <private-key.pk8>".into()),
    }
    Ok(())
}

fn build(root: &Path, version: &str, key_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if !root.is_dir() {
        return Err("Staging directory does not exist".into());
    }
    let key = fs::read(key_path)?;
    let signer =
        Ed25519KeyPair::from_pkcs8(&key).map_err(|_| "Invalid Ed25519 PKCS#8 private key")?;
    let mut files = BTreeMap::new();
    walk(root, root, &mut files)?;
    if files.is_empty() {
        return Err("Staging directory is empty".into());
    }
    for prefix in [".config/opencode/", "tools/"] {
        if !files.keys().any(|name| name.starts_with(prefix)) {
            return Err(format!("Release must include files under {prefix}").into());
        }
    }
    if !files.contains_key("tools/tools-manifest.json") {
        return Err("Missing required release file: tools/tools-manifest.json".into());
    }
    let schema_hashes = files
        .iter()
        .filter(|(name, _)| name.starts_with("schemas/"))
        .map(|(name, artifact)| (name.clone(), artifact.sha256.clone()))
        .collect();
    let manifest = serde_json::to_vec(&Manifest {
        version: version.to_owned(),
        files,
        schema_hashes,
    })?;
    let signature = signer.sign(&manifest);
    fs::write(root.join("release.json"), &manifest)?;
    fs::write(root.join("release.sig"), signature.as_ref())?;
    Ok(())
}

fn walk(root: &Path, dir: &Path, files: &mut BTreeMap<String, Artifact>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Symlinks are not allowed in releases",
            ));
        }
        if kind.is_dir() {
            walk(root, &path, files)?;
            continue;
        }
        if !kind.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unsupported release entry",
            ));
        }
        let name = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if name == "release.json" || name == "release.sig" {
            continue;
        }
        let data = fs::read(&path)?;
        files.insert(
            name,
            Artifact {
                sha256: format!("{:x}", Sha256::digest(&data)),
                bytes: data.len() as u64,
            },
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::{
        rand::SystemRandom,
        signature::{Ed25519KeyPair, UnparsedPublicKey, ED25519},
    };
    use tempfile::tempdir;

    #[test]
    fn build_hashes_final_layout_and_signs_release() {
        let dir = tempdir().unwrap();
        let stage = dir.path().join("staging");
        fs::create_dir_all(stage.join(".config/opencode")).unwrap();
        fs::create_dir_all(stage.join("tools")).unwrap();
        fs::write(stage.join(".config/opencode/opencode.jsonc"), b"{}").unwrap();
        fs::write(stage.join("tools/tool.exe"), b"tool").unwrap();
        fs::write(stage.join("tools/tools-manifest.json"), b"{}").unwrap();
        let pair = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).unwrap();
        let key = dir.path().join("key.pk8");
        fs::write(&key, pair.as_ref()).unwrap();
        build(&stage, "7", &key).unwrap();
        let manifest = fs::read(stage.join("release.json")).unwrap();
        let signature = fs::read(stage.join("release.sig")).unwrap();
        UnparsedPublicKey::new(
            &ED25519,
            Ed25519KeyPair::from_pkcs8(pair.as_ref())
                .unwrap()
                .public_key()
                .as_ref(),
        )
        .verify(&manifest, &signature)
        .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&manifest).unwrap();
        assert_eq!(value["files"]["tools/tool.exe"]["bytes"], 4);
        assert_eq!(
            value["files"][".config/opencode/opencode.jsonc"]["bytes"],
            2
        );
    }
}

use base64::{engine::general_purpose::STANDARD, Engine};
use ring::{
    rand::SystemRandom,
    signature::{Ed25519KeyPair, KeyPair},
};
use si_opencode_updater::{hash_file, safe_name, Artifact, ArtifactDelivery, DeliverySettings, Release};
use std::{collections::BTreeMap, env, fs, io, path::Path};
use serde::Deserialize;

#[derive(Default, Deserialize)]
struct DeliveryConfig {
    #[serde(flatten, default)]
    delivery: DeliverySettings,
    #[serde(default)]
    files: BTreeMap<String, ArtifactDelivery>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("release: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    match args.as_slice() {
        [_, command, private, public] if command == "keygen" => {
            if Path::new(private).exists() || Path::new(public).exists() { return Err("Key already exists".into()); }
            let key = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new())
                .map_err(|_| "Cannot generate signing key")?;
            let pair = Ed25519KeyPair::from_pkcs8(key.as_ref()).map_err(|_| "Cannot parse signing key")?;
            // The key directory must have restricted ACLs; never place the private key in a release folder.
            fs::write(private, key.as_ref())?;
            fs::write(public, pair.public_key().as_ref())?;
        }
        [_, command, private, folder, version, manifest, signature] if command == "sign" => {
            sign(private, folder, version, manifest, signature, None)?;
        }
        [_, command, private, folder, version, manifest, signature, settings] if command == "sign" => {
            sign(private, folder, version, manifest, signature, Some(settings))?;
        }
        [_, command, public, manifest, signature] if command == "verify" => {
            let key = fs::read(public)?;
            let bytes = fs::read(manifest)?;
            let signed = fs::read_to_string(signature)?;
            si_opencode_updater::verify_manifest(&bytes, &signed, &key)?;
        }
        _ => return Err("Usage: release keygen <private.pk8> <public.bin> | release sign <private.pk8> <payload-dir> <version-number> <manifest.json> <manifest.sig>".into()),
    }
    Ok(())
}

fn sign(
    private: &str,
    folder: &str,
    version: &str,
    manifest: &str,
    signature: &str,
    settings: Option<&String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if Path::new(manifest).starts_with(folder) || Path::new(signature).starts_with(folder) {
        return Err("Write manifest and signature outside payload folder".into());
    }
    let key = fs::read(private)?;
    let pair = Ed25519KeyPair::from_pkcs8(&key).map_err(|_| "Invalid private key")?;
    let version: u64 = version.parse()?;
    let mut files = BTreeMap::new();
    collect(Path::new(folder), Path::new(folder), &mut files)?;
    if files.is_empty() {
        return Err("Empty payload".into());
    }
    let config = if let Some(path) = settings {
        serde_json::from_slice::<DeliveryConfig>(&fs::read(path)?)?
    } else {
        DeliveryConfig::default()
    };
    for (path, delivery) in config.files {
        let artifact = files
            .get_mut(&path)
            .ok_or_else(|| format!("Delivery metadata references missing payload file: {path}"))?;
        artifact.delivery = delivery;
    }
    validate_delivery(&files, config.delivery.npm_registry.as_deref())?;
    let bytes = serde_json::to_vec_pretty(&Release {
        schema: 1,
        version,
        files,
        settings: BTreeMap::new(),
        delivery: config.delivery,
    })?;
    let signed = STANDARD.encode(pair.sign(&bytes).as_ref());
    fs::write(manifest, bytes)?;
    fs::write(signature, signed)?;
    Ok(())
}

fn validate_delivery(
    files: &BTreeMap<String, Artifact>,
    default_registry: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    for (path, artifact) in files {
        let delivery = &artifact.delivery;
        let method = delivery.install_method.as_deref().unwrap_or("directCopy");
        if method != "directCopy" && method != "npm" {
            return Err(format!("Unsupported delivery method for {path}: {method}").into());
        }
        if let Some(direct_path) = &delivery.direct_path {
            safe_name(direct_path)?;
        }
        if method == "npm" {
            for (field, value) in [
                ("npmPackage", &delivery.npm_package),
                ("npmVersion", &delivery.npm_version),
                ("npmPath", &delivery.npm_path),
            ] {
                if value.as_deref().is_none_or(str::is_empty) {
                    return Err(format!("{field} is required for npm artifact: {path}").into());
                }
            }
            if delivery
                .npm_registry
                .as_deref()
                .or(default_registry)
                .is_none_or(str::is_empty)
            {
                return Err(format!("npmRegistry is required for npm artifact: {path}").into());
            }
            safe_name(delivery.npm_path.as_deref().unwrap())?;
        }
    }
    Ok(())
}

fn collect(root: &Path, at: &Path, files: &mut BTreeMap<String, Artifact>) -> io::Result<()> {
    for item in fs::read_dir(at)? {
        let item = item?;
        let kind = item.file_type()?;
        if kind.is_symlink() {
            return Err(io::Error::other(
                "Symlinks are not allowed in release payload",
            ));
        }
        if kind.is_dir() {
            collect(root, &item.path(), files)?;
            continue;
        }
        if !kind.is_file() {
            return Err(io::Error::other("Unsupported payload item"));
        }
        let name = item
            .path()
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        safe_name(&name).map_err(io::Error::other)?;
        let size = item.metadata()?.len();
        let hash = hash_file(&item.path())?;
        files.insert(
            name.clone(),
            Artifact {
                sha256: hash,
                bytes: size,
                fpga: name.starts_with("fpga/")
                    || name.split('/').nth(1).is_some_and(|skill| {
                        name.starts_with("skills/")
                            && ["vivado-", "hls-", "hw-", "segcfg-", "vai-"]
                                .iter()
                                .any(|prefix| skill.starts_with(prefix))
                    }),
                delivery: ArtifactDelivery::default(),
            },
        );
    }
    Ok(())
}

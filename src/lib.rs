use base64::{engine::general_purpose::STANDARD, Engine};
use ring::signature::{self, UnparsedPublicKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Component, Path},
};

pub mod config;
#[cfg(windows)]
pub mod credentials;
pub mod nas;
pub mod provision;
pub mod release_notes;
pub mod validate;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Release {
    pub schema: u32,
    pub version: u64,
    pub files: BTreeMap<String, Artifact>,
    #[serde(default)]
    pub settings: BTreeMap<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "DeliverySettings::is_empty")]
    pub delivery: DeliverySettings,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeliverySettings {
    #[serde(rename = "npmRegistry", skip_serializing_if = "Option::is_none")]
    pub npm_registry: Option<String>,
}

impl DeliverySettings {
    fn is_empty(settings: &Self) -> bool {
        settings.npm_registry.is_none()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactDelivery {
    #[serde(rename = "installMethod", skip_serializing_if = "Option::is_none")]
    pub install_method: Option<String>,
    #[serde(rename = "npmPackage", skip_serializing_if = "Option::is_none")]
    pub npm_package: Option<String>,
    #[serde(rename = "npmVersion", skip_serializing_if = "Option::is_none")]
    pub npm_version: Option<String>,
    #[serde(rename = "npmRegistry", skip_serializing_if = "Option::is_none")]
    pub npm_registry: Option<String>,
    #[serde(rename = "npmPath", skip_serializing_if = "Option::is_none")]
    pub npm_path: Option<String>,
    #[serde(rename = "fallbackToDirectCopy", skip_serializing_if = "Option::is_none")]
    pub fallback_to_direct_copy: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional: Option<bool>,
    #[serde(rename = "requiresAdmin", skip_serializing_if = "Option::is_none")]
    pub requires_admin: Option<bool>,
    #[serde(rename = "directPath", skip_serializing_if = "Option::is_none")]
    pub direct_path: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Artifact {
    pub sha256: String,
    pub bytes: u64,
    pub fpga: bool,
    #[serde(flatten)]
    pub delivery: ArtifactDelivery,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Installed {
    pub version: u64,
    // Only files previously written by this installer/updater are in this map.
    pub files: BTreeMap<String, String>,
    #[serde(default)]
    pub config: BTreeMap<String, String>,
    #[serde(default)]
    pub published_config: BTreeMap<String, String>,
    #[serde(default)]
    pub desktop_sha256: Option<String>,
    #[serde(default)]
    pub context_mode_plugin: Option<String>,
    #[serde(default)]
    pub shown_release_notes: Option<u64>,
}

/// This is installed as a user-editable local file, not embedded in the updater executable.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NasConfig {
    pub host: String,
    pub manifest_path: String,
}

impl NasConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.host != "https://file.satreci.com" {
            return Err("NAS host must be https://file.satreci.com".into());
        }
        if !self.manifest_path.starts_with('/')
            || self.manifest_path.contains(['?', '#', '\\', ':'])
        {
            return Err("Provide a NAS FileStation path, not a download link".into());
        }
        safe_name(&self.manifest_path[1..])
    }
}

pub fn verify_manifest(
    bytes: &[u8],
    signature_b64: &str,
    public_key: &[u8],
) -> Result<Release, String> {
    let signature = STANDARD
        .decode(signature_b64.trim())
        .map_err(|e| e.to_string())?;
    UnparsedPublicKey::new(&signature::ED25519, public_key)
        .verify(bytes, &signature)
        .map_err(|_| "Invalid release signature".to_string())?;
    let release: Release = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if release.schema != 1 {
        return Err("Unsupported manifest schema".into());
    }
    let mut seen = std::collections::HashSet::new();
    for (name, artifact) in &release.files {
        safe_name(name)?;
        if !seen.insert(name.to_ascii_lowercase()) {
            return Err("Case-insensitive path collision".into());
        }
        if artifact.sha256.len() != 64 || !artifact.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("Invalid digest for {name}"));
        }
    }
    for key in release.settings.keys() {
        if !key.starts_with("mcp.servers.")
            || matches!(
                key.as_str(),
                "mcp.servers.wiki" | "mcp.servers.work" | "mcp.servers.code" | "mcp.servers.vivado"
            )
            || key.split('.').count() != 3
        {
            return Err(format!(
                "Manifest may not replace user credentials or unrelated settings: {key}"
            ));
        }
    }
    Ok(release)
}

pub fn safe_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.contains(['\\', ':'])
        || name.to_ascii_lowercase().starts_with(".si-update-")
        || name.split('/').any(|part| {
            part.is_empty() || part == "." || part == ".." || part.ends_with([' ', '.'])
        })
        || name.split('/').any(|part| {
            let stem = part
                .split('.')
                .next()
                .unwrap_or_default()
                .to_ascii_uppercase();
            matches!(
                stem.as_str(),
                "CON"
                    | "PRN"
                    | "AUX"
                    | "NUL"
                    | "COM1"
                    | "COM2"
                    | "COM3"
                    | "COM4"
                    | "COM5"
                    | "COM6"
                    | "COM7"
                    | "COM8"
                    | "COM9"
                    | "LPT1"
                    | "LPT2"
                    | "LPT3"
                    | "LPT4"
                    | "LPT5"
                    | "LPT6"
                    | "LPT7"
                    | "LPT8"
                    | "LPT9"
            )
        })
        || !Path::new(name)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err(format!("Unsafe artifact path: {name}"));
    }
    Ok(())
}

pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn check_payload(artifact: &Artifact, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() as u64 != artifact.bytes || sha256(bytes) != artifact.sha256.to_ascii_lowercase()
    {
        return Err("Payload size or hash mismatch".into());
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    Install(String),
    Remove(String),
    Conflict(String),
}

#[derive(Serialize, Deserialize)]
struct Journal {
    old: Installed,
    actions: Vec<Action>,
    release: Release,
}

fn write_state(root: &Path, state: &Installed) -> io::Result<()> {
    let tmp = root.join(".si-update-state-new");
    let path = root.join(".si-update-state.json");
    let mut file = fs::File::create(&tmp)?;
    serde_json::to_writer(&mut file, state).map_err(io::Error::other)?;
    file.sync_all()?;
    drop(file);
    if path.exists() {
        fs::remove_file(&path)?;
    }
    fs::rename(tmp, path)
}

pub fn save_state(root: &Path, state: &Installed) -> io::Result<()> {
    write_state(root, state)
}

pub fn load_state(root: &Path) -> io::Result<Installed> {
    recover(root)?;
    match fs::read(root.join(".si-update-state.json")) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Installed::default()),
        Err(error) => Err(error),
    }
}

pub fn recover(root: &Path) -> io::Result<()> {
    let path = root.join(".si-update-journal.json");
    if !path.exists() {
        let backup = root.join(".si-update-backup");
        if backup.exists() && root.join(".si-update-state.json").exists() {
            fs::remove_dir_all(backup)?;
        }
        return Ok(());
    }
    let journal: Journal = serde_json::from_slice(&fs::read(&path)?).map_err(io::Error::other)?;
    for action in journal.actions.iter().rev() {
        let name = match action {
            Action::Install(n) | Action::Remove(n) => n,
            Action::Conflict(_) => continue,
        };
        safe_name(name).map_err(io::Error::other)?;
        check_no_links(root, name)?;
        let target = root.join(name);
        let saved = root.join(".si-update-backup").join(name);
        if saved.exists() {
            if target.exists() {
                let expected = journal.release.files.get(name).ok_or_else(|| {
                    io::Error::other(format!(
                        "User recreated {name} during recovery; manual intervention required"
                    ))
                })?;
                if hash_file(&target)? != expected.sha256 {
                    return Err(io::Error::other(format!(
                        "User changed {name} during recovery; manual intervention required"
                    )));
                }
                fs::remove_file(&target)?;
            }
            fs::create_dir_all(target.parent().unwrap())?;
            fs::rename(saved, target)?;
        } else if let Action::Install(_) = action {
            if target.exists() && !journal.old.files.contains_key(name) {
                if hash_file(&target)? != journal.release.files[name].sha256 {
                    return Err(io::Error::other(format!(
                        "User changed {name} during recovery; manual intervention required"
                    )));
                }
                fs::remove_file(target)?;
            }
        }
    }
    write_state(root, &journal.old)?;
    fs::remove_file(path)?;
    fs::remove_dir_all(root.join(".si-update-backup"))?;
    Ok(())
}

pub fn plan(root: &Path, old: &Installed, new: &Release, fpga: bool) -> io::Result<Vec<Action>> {
    let mut actions = Vec::new();
    for (name, old_hash) in &old.files {
        safe_name(name).map_err(io::Error::other)?;
        check_no_links(root, name)?;
        let wanted = new.files.get(name).filter(|entry| fpga || !entry.fpga);
        if wanted.is_none() {
            let path = root.join(name);
            if path.is_file() && hash_file(&path)? == *old_hash {
                actions.push(Action::Remove(name.clone()));
            } else {
                actions.push(Action::Conflict(name.clone()));
            }
        }
    }
    for (name, artifact) in &new.files {
        if artifact.fpga && !fpga {
            continue;
        }
        safe_name(name).map_err(io::Error::other)?;
        check_no_links(root, name)?;
        let path = root.join(name);
        if path.exists() {
            let actual = hash_file(&path)?;
            if actual == artifact.sha256 {
                continue;
            }
            if old.files.get(name) != Some(&actual) {
                actions.push(Action::Conflict(name.clone()));
                continue;
            }
        }
        actions.push(Action::Install(name.clone()));
    }
    Ok(actions)
}

fn check_no_links(root: &Path, name: &str) -> io::Result<()> {
    let mut current = root.to_path_buf();
    for component in Path::new(name).components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if metadata.file_attributes() & 0x400 != 0 {
                        return Err(io::Error::other(format!(
                            "Reparse point in managed path: {name}"
                        )));
                    }
                }
                if metadata.file_type().is_symlink() {
                    return Err(io::Error::other("Symlink in managed path"));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => break,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

pub fn hash_file(path: &Path) -> io::Result<String> {
    use std::io::Read;
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

// Staging and backup directories must be on the same volume as the installation.
pub fn apply(
    root: &Path,
    staging: &Path,
    old: &mut Installed,
    new: &Release,
    fpga: bool,
) -> io::Result<Vec<String>> {
    recover(root)?;
    if new.version < old.version {
        return Err(io::Error::other("Refusing release downgrade"));
    }
    let actions = plan(root, old, new, fpga)?;
    let conflicts: Vec<_> = actions
        .iter()
        .filter_map(|action| match action {
            Action::Conflict(n) => Some(n.clone()),
            _ => None,
        })
        .collect();
    let changes: Vec<_> = actions
        .iter()
        .filter(|action| !matches!(action, Action::Conflict(_)))
        .collect();
    for action in &changes {
        if let Action::Install(name) = action {
            check_no_links(staging, name)?;
            let artifact = &new.files[name];
            let path = staging.join(name);
            if fs::metadata(&path)?.len() != artifact.bytes
                || hash_file(&path)? != artifact.sha256.to_ascii_lowercase()
            {
                return Err(io::Error::other(format!(
                    "Payload size or hash mismatch: {name}"
                )));
            }
        }
    }
    let backup = root.join(".si-update-backup");
    if backup.exists() {
        return Err(io::Error::other("Unfinished backup: recover before retry"));
    }
    fs::create_dir_all(&backup)?;
    let journal = Journal {
        old: old.clone(),
        actions: changes.iter().map(|action| (*action).clone()).collect(),
        release: new.clone(),
    };
    let journal_path = root.join(".si-update-journal.json");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&journal_path)?;
    serde_json::to_writer(&mut file, &journal).map_err(io::Error::other)?;
    file.sync_all()?;
    drop(file);
    let result = (|| -> io::Result<()> {
        for action in &changes {
            let name = match action {
                Action::Install(n) | Action::Remove(n) => n,
                Action::Conflict(_) => unreachable!(),
            };
            let target = root.join(name);
            if target.exists() {
                let saved = backup.join(name);
                fs::create_dir_all(saved.parent().unwrap())?;
                fs::rename(&target, &saved)?;
            }
            if matches!(action, Action::Install(_)) {
                fs::create_dir_all(target.parent().unwrap())?;
                fs::rename(staging.join(name), &target)?;
            }
        }
        Ok(())
    })();
    if let Err(error) = result {
        recover(root)?;
        return Err(error);
    }
    for action in changes {
        match action {
            Action::Install(name) => {
                old.files
                    .insert(name.clone(), new.files[name].sha256.clone());
            }
            Action::Remove(name) => {
                old.files.remove(name);
            }
            Action::Conflict(_) => unreachable!(),
        }
    }
    // Keep version behind when conflicts remain, so the next check can retry them.
    if conflicts.is_empty() {
        old.version = new.version;
    }
    if let Err(error) = write_state(root, old) {
        recover(root)?;
        *old = journal.old;
        return Err(error);
    }
    fs::remove_file(journal_path)?;
    fs::remove_dir_all(backup)?;
    Ok(conflicts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::{
        rand::SystemRandom,
        signature::{Ed25519KeyPair, KeyPair},
    };

    #[test]
    fn signed_manifest_rejects_tampering_and_traversal() {
        let rng = SystemRandom::new();
        let key = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        let key = Ed25519KeyPair::from_pkcs8(key.as_ref()).unwrap();
        let release = Release {
            schema: 1,
            version: 2,
            files: BTreeMap::new(),
            settings: BTreeMap::new(),
            delivery: DeliverySettings::default(),
        };
        let bytes = serde_json::to_vec(&release).unwrap();
        let sig = STANDARD.encode(key.sign(&bytes).as_ref());
        assert!(verify_manifest(&bytes, &sig, key.public_key().as_ref()).is_ok());
        assert!(verify_manifest(b"{}", &sig, key.public_key().as_ref()).is_err());
        assert!(safe_name("skills/../secrets").is_err());
        assert!(safe_name("C:\\Users\\secret").is_err());
        assert!(safe_name("CON.txt").is_err());
    }

    #[test]
    fn artifact_delivery_metadata_is_part_of_the_signed_manifest_json() {
        let artifact = Artifact {
            sha256: sha256(b"npm"),
            bytes: 3,
            fpga: false,
            delivery: ArtifactDelivery {
                install_method: Some("npm".into()),
                npm_package: Some("@satreci/example".into()),
                npm_version: Some("1.0.0".into()),
                npm_registry: Some("https://nexus.example/repository/npm-group/".into()),
                npm_path: Some("dist/example.js".into()),
                fallback_to_direct_copy: Some(true),
                optional: Some(false),
                requires_admin: None,
                direct_path: None,
            },
        };
        let bytes = serde_json::to_vec(&artifact).unwrap();
        assert!(String::from_utf8(bytes.clone()).unwrap().contains("\"installMethod\":\"npm\""));
        assert_eq!(serde_json::from_slice::<Artifact>(&bytes).unwrap(), artifact);
    }

    #[test]
    fn nas_path_is_configurable_but_never_an_arbitrary_credential_destination() {
        let mut config = NasConfig {
            host: "https://file.satreci.com".into(),
            manifest_path: "/si_우주과제/si_opencode/manifest.json".into(),
        };
        assert!(config.validate().is_ok());
        config.host = "https://attacker.example".into();
        assert!(config.validate().is_err());
        config.host = "https://file.satreci.com".into();
        config.manifest_path = "/si_우주과제/../secret".into();
        assert!(config.validate().is_err());
        config.manifest_path = "/fbdownload/item?SynoToken=secret".into();
        assert!(config.validate().is_err());
    }

    #[test]
    fn removal_respects_user_changes_and_new_files() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("owned"), b"edited").unwrap();
        fs::write(tmp.path().join("personal"), b"personal").unwrap();
        let old = Installed {
            version: 1,
            files: BTreeMap::from([("owned".into(), sha256(b"original"))]),
            config: BTreeMap::new(),
            published_config: BTreeMap::new(),
            desktop_sha256: None,
            context_mode_plugin: None,
            shown_release_notes: None,
        };
        let new = Release {
            schema: 1,
            version: 2,
            files: BTreeMap::new(),
            settings: BTreeMap::new(),
            delivery: DeliverySettings::default(),
        };
        assert_eq!(
            plan(tmp.path(), &old, &new, false).unwrap(),
            vec![Action::Conflict("owned".into())]
        );
        assert_eq!(fs::read(tmp.path().join("personal")).unwrap(), b"personal");
    }

    #[test]
    fn verifies_all_payloads_before_changing_anything() {
        let root = tempfile::tempdir().unwrap();
        let stage = tempfile::tempdir().unwrap();
        fs::write(root.path().join("existing"), b"old").unwrap();
        let mut old = Installed {
            version: 1,
            files: BTreeMap::from([("existing".into(), sha256(b"old"))]),
            config: BTreeMap::new(),
            published_config: BTreeMap::new(),
            desktop_sha256: None,
            context_mode_plugin: None,
            shown_release_notes: None,
        };
        let new = Release {
            schema: 1,
            version: 2,
            files: BTreeMap::from([(
                "new".into(),
                Artifact {
                    sha256: sha256(b"good"),
                    bytes: 4,
                    fpga: false,
                    delivery: ArtifactDelivery::default(),
                },
            )]),
            settings: BTreeMap::new(),
            delivery: DeliverySettings::default(),
        };
        fs::write(stage.path().join("new"), b"bad!").unwrap();
        assert!(apply(root.path(), stage.path(), &mut old, &new, false).is_err());
        assert_eq!(fs::read(root.path().join("existing")).unwrap(), b"old");
        assert_eq!(old.version, 1);
    }

    #[test]
    fn update_only_touches_owned_files_and_respects_fpga() {
        let root = tempfile::tempdir().unwrap();
        let stage = tempfile::tempdir().unwrap();
        fs::write(root.path().join("old"), b"v1").unwrap();
        fs::write(root.path().join("personal"), b"mine").unwrap();
        fs::write(stage.path().join("new"), b"v2").unwrap();
        let mut old = Installed {
            version: 1,
            files: BTreeMap::from([("old".into(), sha256(b"v1"))]),
            config: BTreeMap::new(),
            published_config: BTreeMap::new(),
            desktop_sha256: None,
            context_mode_plugin: None,
            shown_release_notes: None,
        };
        let new = Release {
            schema: 1,
            version: 2,
            files: BTreeMap::from([
                (
                    "new".into(),
                    Artifact {
                        sha256: sha256(b"v2"),
                        bytes: 2,
                        fpga: false,
                        delivery: ArtifactDelivery::default(),
                    },
                ),
                (
                    "fpga/vivado".into(),
                    Artifact {
                        sha256: sha256(b"kb"),
                        bytes: 2,
                        fpga: true,
                        delivery: ArtifactDelivery::default(),
                    },
                ),
            ]),
            settings: BTreeMap::new(),
            delivery: DeliverySettings::default(),
        };
        assert!(apply(root.path(), stage.path(), &mut old, &new, false)
            .unwrap()
            .is_empty());
        assert!(!root.path().join("old").exists());
        assert_eq!(fs::read(root.path().join("new")).unwrap(), b"v2");
        assert_eq!(fs::read(root.path().join("personal")).unwrap(), b"mine");
        assert!(!root.path().join("fpga").exists());
        assert_eq!(old.version, 2);
        assert!(apply(
            root.path(),
            stage.path(),
            &mut old,
            &Release { version: 1, ..new },
            false
        )
        .is_err());
    }

    #[test]
    fn restart_recovers_interrupted_file_replacement() {
        let root = tempfile::tempdir().unwrap();
        let old = Installed {
            version: 1,
            files: BTreeMap::from([("plugin".into(), sha256(b"old"))]),
            config: BTreeMap::new(),
            published_config: BTreeMap::new(),
            desktop_sha256: None,
            context_mode_plugin: None,
            shown_release_notes: None,
        };
        let new = Release {
            schema: 1,
            version: 2,
            files: BTreeMap::from([(
                "plugin".into(),
                Artifact {
                    sha256: sha256(b"new"),
                    bytes: 3,
                    fpga: false,
                    delivery: ArtifactDelivery::default(),
                },
            )]),
            settings: BTreeMap::new(),
            delivery: DeliverySettings::default(),
        };
        let backup = root.path().join(".si-update-backup");
        fs::create_dir(&backup).unwrap();
        fs::write(backup.join("plugin"), b"old").unwrap();
        fs::write(root.path().join("plugin"), b"new").unwrap();
        fs::write(
            root.path().join(".si-update-journal.json"),
            serde_json::to_vec(&Journal {
                old: old.clone(),
                actions: vec![Action::Install("plugin".into())],
                release: new,
            })
            .unwrap(),
        )
        .unwrap();
        let restored = load_state(root.path()).unwrap();
        assert_eq!(restored.version, 1);
        assert_eq!(fs::read(root.path().join("plugin")).unwrap(), b"old");
        assert!(!backup.exists());
    }
}

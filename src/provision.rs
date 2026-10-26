use crate::{config, credentials, hash_file, nas::Nas, safe_name, Artifact, NasConfig};
use serde::Deserialize;
use std::{collections::BTreeMap, env, fs, io, path::{Path, PathBuf}};

#[derive(Deserialize)]
struct Input {
    api_key: String,
    wiki_pat: String,
    work_pat: String,
    #[serde(default)]
    code_pat: String,
}

pub fn install(input_file: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = fs::read(input_file)?;
    fs::remove_file(input_file)?;
    let input: Input = serde_json::from_slice(&bytes)?;
    if [input.api_key.as_str(), &input.wiki_pat, &input.work_pat].iter().any(|value| value.trim().is_empty()) {
        return Err("LLM key, wiki PAT and work PAT are mandatory".into());
    }
    let chat_url = crate::validate::llm(&input.api_key)?;
    let base_url = chat_url.strip_suffix("/chat/completions").ok_or("Unexpected LLM endpoint")?;
    crate::validate::mcp("https://dxdev.satreci.com/mcpwiki/mcp", &input.wiki_pat)?;
    crate::validate::mcp("https://dxdev.satreci.com/mcpwork/mcp", &input.work_pat)?;
    let code = if input.code_pat.trim().is_empty() { None } else { Some(input.code_pat.as_str()) };

    let root = PathBuf::from(env::var("USERPROFILE")?).join(".config/opencode");
    fs::create_dir_all(&root)?;
    let config_file = config_path(&root);
    let previous = match fs::read(&config_file) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    let mut current = match &previous {
        Some(bytes) => config::parse_jsonc(std::str::from_utf8(bytes)?)?,
        None => serde_json::json!({"$schema":"https://opencode.ai/config.json"}),
    };
    let values = config::initial(&input.api_key, &input.wiki_pat, &input.work_pat, code, base_url, &root);
    config::install_values(&mut current, &mut BTreeMap::new(), &values)?;
    fs::write(&config_file, serde_json::to_vec_pretty(&current)?)?;
    Ok(())
}

pub fn config_path(root: &Path) -> PathBuf {
    let jsonc = root.join("opencode.jsonc");
    if jsonc.exists() { jsonc } else {
        let json = root.join("opencode.json");
        if json.exists() { json } else { jsonc }
    }
}

#[derive(Deserialize)]
struct ExtensionManifest {
    files: BTreeMap<String, Artifact>,
    #[serde(default)]
    settings: BTreeMap<String, serde_json::Value>,
}

pub fn sync_extensions() -> Result<(), Box<dyn std::error::Error>> {
    let base = PathBuf::from(env::var("LOCALAPPDATA")?).join("SI OpenCode");
    let config: NasConfig = serde_json::from_slice(&fs::read(base.join("extensions.json"))?)?;
    config.validate()?;
    let account = match credentials::load()? {
        Some(account) => account,
        None => credentials::prompt()?.ok_or("NAS sign-in cancelled")?,
    };
    let nas = Nas::login(&config, &account.0, &account.1)?;
    credentials::save(&account.0, &account.1)?;
    let temp = tempfile::tempdir()?;
    let manifest_file = temp.path().join("extensions-manifest.json");
    nas.download(&config.manifest_path, &manifest_file, 4 * 1024 * 1024, None)?;
    let manifest: ExtensionManifest = serde_json::from_slice(&fs::read(&manifest_file)?)?;
    let root = PathBuf::from(env::var("USERPROFILE")?).join(".config/opencode");
    fs::create_dir_all(&root)?;
    let previous_path = root.join(".si-extension-files.json");
    let previous: BTreeMap<String, String> = fs::read(&previous_path).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default();
    let mut next = BTreeMap::new();
    for (name, artifact) in &manifest.files {
        safe_name(name)?;
        if !["mcp/", "plugin/", "skills/", "lsp/"].iter().any(|prefix| name.starts_with(prefix)) { return Err(format!("Extension path is outside allowed folders: {name}").into()); }
        let target = root.join(name);
        if target.is_file() && hash_file(&target)? == artifact.sha256 { next.insert(name.clone(), artifact.sha256.clone()); continue; }
        let stage = temp.path().join(name);
        fs::create_dir_all(stage.parent().ok_or("Invalid extension path")?)?;
        let remote = format!("{}/{}", config.manifest_path.rsplit_once('/').ok_or("Invalid manifest path")?.0, name);
        nas.download(&remote, &stage, artifact.bytes, Some(artifact))?;
        fs::create_dir_all(target.parent().ok_or("Invalid extension path")?)?;
        if target.exists() && previous.get(name) == Some(&hash_file(&target)?) { fs::remove_file(&target)?; }
        if !target.exists() { fs::rename(stage, &target)?; }
        next.insert(name.clone(), artifact.sha256.clone());
    }
    for (name, hash) in previous {
        if next.contains_key(&name) { continue; }
        let target = root.join(&name);
        if target.is_file() && hash_file(&target)? == hash { fs::remove_file(target)?; }
    }
    fs::write(previous_path, serde_json::to_vec_pretty(&next)?)?;
    let path = config_path(&root);
    let mut current = config::parse_jsonc(std::str::from_utf8(&fs::read(&path)?)?)?;
    let settings_state = root.join(".si-extension-settings.json");
    let mut previous_settings: BTreeMap<String, String> = fs::read(&settings_state).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default();
    let settings = manifest.settings.into_iter().map(|(key, value)| (key, expand_root(value, &root))).collect();
    let conflicts = config::reconcile(&mut current, &mut previous_settings, &settings)?;
    if !conflicts.is_empty() { eprintln!("Preserved user-edited extension settings: {}", conflicts.join(", ")); }
    fs::write(path, serde_json::to_vec_pretty(&current)?)?;
    fs::write(settings_state, serde_json::to_vec_pretty(&previous_settings)?)?;
    let _ = nas.logout();
    Ok(())
}

fn expand_root(value: serde_json::Value, root: &Path) -> serde_json::Value {
    match value {
        serde_json::Value::String(text) => serde_json::Value::String(text.replace("{root}", &root.to_string_lossy())),
        serde_json::Value::Array(items) => serde_json::Value::Array(items.into_iter().map(|item| expand_root(item, root)).collect()),
        serde_json::Value::Object(items) => serde_json::Value::Object(items.into_iter().map(|(key, item)| (key, expand_root(item, root))).collect()),
        other => other,
    }
}

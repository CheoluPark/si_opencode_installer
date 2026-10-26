use crate::{config, credentials, hash_file, nas::Nas, safe_name, Artifact, NasConfig};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    env, fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Deserialize)]
struct Input {
    api_key: String,
    wiki_pat: String,
    work_pat: String,
    #[serde(default)]
    code_pat: String,
}

pub fn install(
    input_file: &Path,
    staging: &Path,
    overwrite_confirmed: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = fs::read(input_file)?;
    fs::remove_file(input_file)?;
    let input: Input = serde_json::from_slice(&bytes)?;
    if [input.api_key.as_str(), &input.wiki_pat, &input.work_pat]
        .iter()
        .any(|value| value.trim().is_empty())
    {
        return Err("LLM key, wiki PAT and work PAT are mandatory".into());
    }
    let chat_url = crate::validate::llm(&input.api_key)?;
    let base_url = chat_url
        .strip_suffix("/chat/completions")
        .ok_or("Unexpected LLM endpoint")?;
    crate::validate::mcp("https://dxdev.satreci.com/mcpwiki/mcp", &input.wiki_pat)?;
    crate::validate::mcp("https://dxdev.satreci.com/mcpwork/mcp", &input.work_pat)?;
    let code = if input.code_pat.trim().is_empty() {
        None
    } else {
        Some(input.code_pat.as_str())
    };
    if let Some(code) = code {
        crate::validate::mcp("https://dxdev.satreci.com/mcpcode/mcp", code)?;
    }

    let profile = PathBuf::from(env::var("USERPROFILE")?);
    let root = profile.join(".config/opencode");
    let bundle = staging.join(".config/opencode");
    if !bundle.is_dir() {
        return Err("Staged release has no .config/opencode bundle".into());
    }
    if root.exists() && !overwrite_confirmed {
        return Err("Existing OpenCode config requires explicit overwrite confirmation".into());
    }
    let profile_text = profile.to_string_lossy();
    let replacements = BTreeMap::from([
        ("{{API_KEY}}", input.api_key.as_str()),
        ("{{WIKI_PAT}}", input.wiki_pat.as_str()),
        ("{{WORK_PAT}}", input.work_pat.as_str()),
        ("{{CODE_PAT}}", code.unwrap_or("")),
        ("{{USERPROFILE}}", profile_text.as_ref()),
        ("{{LLM_BASE_URL}}", base_url),
    ]);
    let generated = tempfile::tempdir()?;
    render_bundle(&bundle, generated.path(), &replacements, code.is_some())?;
    let mut config_found = false;
    for path in walk_files(generated.path())? {
        if path
            .file_name()
            .is_some_and(|name| name == "opencode.json" || name == "opencode.jsonc")
        {
            config_found = true;
            let text = fs::read_to_string(&path)?;
            let value = config::parse_jsonc(&text)?;
            validate_v2(&value)?;
        }
    }
    if !config_found {
        return Err("Staged config bundle has no opencode.json or opencode.jsonc".into());
    }
    deploy_bundle(generated.path(), &root, overwrite_confirmed)?;
    Ok(())
}

fn deploy_bundle(
    source: &Path,
    root: &Path,
    overwrite_confirmed: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if root.exists() && !overwrite_confirmed {
        return Err("Existing OpenCode config requires explicit overwrite confirmation".into());
    }
    if let Some(parent) = root.parent() {
        fs::create_dir_all(parent)?;
    }
    let deploy = root.with_file_name(format!(
        "opencode.deploy-{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    copy_tree(source, &deploy)?;
    if root.exists() {
        let backup = root.with_file_name(format!(
            "opencode.backup-{}",
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        ));
        fs::rename(root, &backup)?;
        if let Err(error) = fs::rename(&deploy, root) {
            let _ = fs::rename(&backup, root);
            return Err(error.into());
        }
    } else if let Err(error) = fs::rename(&deploy, root) {
        let _ = fs::remove_dir_all(&deploy);
        return Err(error.into());
    }
    Ok(())
}

fn render_bundle(
    source: &Path,
    dest: &Path,
    vars: &BTreeMap<&str, &str>,
    include_code: bool,
) -> io::Result<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if from.is_dir() {
            render_bundle(&from, &to, vars, include_code)?;
        } else {
            let bytes = fs::read(&from)?;
            if let Ok(mut text) = String::from_utf8(bytes.clone()) {
                const BEGIN: &str = "// SI:CODE_PAT:BEGIN";
                const END: &str = "// SI:CODE_PAT:END";
                if text.contains(BEGIN) != text.contains(END) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Unbalanced optional code PAT template markers",
                    ));
                }
                if !include_code {
                    if let (Some(start), Some(end)) = (text.find(BEGIN), text.find(END)) {
                        text.replace_range(start..end + END.len(), "");
                    }
                }
                for (marker, value) in vars {
                    let encoded = serde_json::to_string(value).unwrap();
                    text = text.replace(marker, &encoded[1..encoded.len() - 1]);
                }
                if text.contains("{{") {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Unresolved template marker in {}", from.display()),
                    ));
                }
                fs::write(to, text)?;
            } else {
                fs::write(to, bytes)?;
            }
        }
    }
    Ok(())
}
fn walk_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.is_dir() {
            files.extend(walk_files(&path)?);
        } else {
            files.push(path);
        }
    }
    Ok(files)
}
fn copy_tree(source: &Path, dest: &Path) -> io::Result<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if from.is_dir() {
            copy_tree(&from, &to)?;
        } else {
            fs::copy(from, to)?;
        }
    }
    Ok(())
}
fn validate_v2(value: &serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../config/opencode-v2.schema.json"))?;
    validate_schema(value, &schema, "$")?;
    Ok(())
}
fn validate_schema(
    value: &serde_json::Value,
    schema: &serde_json::Value,
    at: &str,
) -> Result<(), String> {
    let matches_type = |kind: &str| match kind {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "boolean" => value.is_boolean(),
        "integer" => value.as_i64().is_some(),
        "number" => value.is_number(),
        "null" => value.is_null(),
        _ => true,
    };
    let accepted = schema["type"]
        .as_str()
        .map(matches_type)
        .or_else(|| {
            schema["type"].as_array().map(|types| {
                types
                    .iter()
                    .any(|kind| kind.as_str().is_some_and(matches_type))
            })
        })
        .unwrap_or(true);
    if !accepted {
        return Err(format!("OpenCode v2 schema: {at} has the wrong type"));
    }
    if let Some(required) = schema["required"].as_array() {
        for key in required {
            if let Some(key) = key.as_str() {
                if value.get(key).is_none() {
                    return Err(format!("OpenCode v2 schema: {at}.{key} is required"));
                }
            }
        }
    }
    if let Some(map) = value.as_object() {
        if let Some(props) = schema["properties"].as_object() {
            for (key, child) in map {
                if let Some(sub) = props.get(key) {
                    validate_schema(child, sub, &format!("{at}.{key}"))?;
                } else if schema["additionalProperties"] == false {
                    return Err(format!(
                        "OpenCode v2 schema: unexpected property {at}.{key}"
                    ));
                }
            }
        }
    }
    if let (Some(items), Some(item_schema)) = (value.as_array(), schema.get("items")) {
        for (index, child) in items.iter().enumerate() {
            validate_schema(child, item_schema, &format!("{at}[{index}]"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod deployment_tests {
    use super::*;
    #[test]
    fn renders_markers_and_removes_optional_code_region() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let output = temp.path().join("out");
        fs::create_dir_all(&source).unwrap();
        fs::write(
            source.join("opencode.jsonc"),
            r#"{"key":"{{API_KEY}}", // SI:CODE_PAT:BEGIN
"code":"{{CODE_PAT}}",
// SI:CODE_PAT:END
"profile":"{{USERPROFILE}}"}"#,
        )
        .unwrap();
        render_bundle(
            &source,
            &output,
            &BTreeMap::from([
                ("{{API_KEY}}", "a\"b"),
                ("{{CODE_PAT}}", ""),
                ("{{USERPROFILE}}", "C:\\Users\\me"),
            ]),
            false,
        )
        .unwrap();
        let result = fs::read_to_string(output.join("opencode.jsonc")).unwrap();
        assert!(!result.contains("SI:CODE_PAT"));
        assert!(!result.contains("{{"));
        assert!(config::parse_jsonc(&result).is_ok());
    }
    #[test]
    fn bundled_schema_checks_config_value_types() {
        let valid =
            serde_json::json!({"$schema":"https://opencode.ai/config.json", "model":"si/GPT"});
        validate_v2(&valid).unwrap();
        let invalid = serde_json::json!({"model":3});
        assert!(validate_v2(&invalid).is_err());
    }
    #[test]
    fn deploy_requires_confirmation_and_preserves_backup() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("opencode");
        let source = temp.path().join("bundle");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&source).unwrap();
        fs::write(root.join("user.txt"), "preserve").unwrap();
        fs::write(source.join("opencode.jsonc"), "{}").unwrap();
        assert!(deploy_bundle(&source, &root, false).is_err());
        deploy_bundle(&source, &root, true).unwrap();
        assert!(root.join("opencode.jsonc").is_file());
        let backup = fs::read_dir(temp.path())
            .unwrap()
            .flatten()
            .find(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("opencode.backup-")
            })
            .unwrap();
        assert_eq!(
            fs::read_to_string(backup.path().join("user.txt")).unwrap(),
            "preserve"
        );
    }
}

pub fn config_path(root: &Path) -> PathBuf {
    let jsonc = root.join("opencode.jsonc");
    if jsonc.exists() {
        jsonc
    } else {
        let json = root.join("opencode.json");
        if json.exists() {
            json
        } else {
            jsonc
        }
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
    let previous: BTreeMap<String, String> = fs::read(&previous_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let mut next = BTreeMap::new();
    for (name, artifact) in &manifest.files {
        safe_name(name)?;
        if !["mcp/", "plugin/", "skills/", "lsp/"]
            .iter()
            .any(|prefix| name.starts_with(prefix))
        {
            return Err(format!("Extension path is outside allowed folders: {name}").into());
        }
        let target = root.join(name);
        if target.is_file() && hash_file(&target)? == artifact.sha256 {
            next.insert(name.clone(), artifact.sha256.clone());
            continue;
        }
        let stage = temp.path().join(name);
        fs::create_dir_all(stage.parent().ok_or("Invalid extension path")?)?;
        let remote = format!(
            "{}/{}",
            config
                .manifest_path
                .rsplit_once('/')
                .ok_or("Invalid manifest path")?
                .0,
            name
        );
        nas.download(&remote, &stage, artifact.bytes, Some(artifact))?;
        fs::create_dir_all(target.parent().ok_or("Invalid extension path")?)?;
        if target.exists() && previous.get(name) == Some(&hash_file(&target)?) {
            fs::remove_file(&target)?;
        }
        if !target.exists() {
            fs::rename(stage, &target)?;
        }
        next.insert(name.clone(), artifact.sha256.clone());
    }
    for (name, hash) in previous {
        if next.contains_key(&name) {
            continue;
        }
        let target = root.join(&name);
        if target.is_file() && hash_file(&target)? == hash {
            fs::remove_file(target)?;
        }
    }
    fs::write(previous_path, serde_json::to_vec_pretty(&next)?)?;
    let path = config_path(&root);
    let mut current = config::parse_jsonc(std::str::from_utf8(&fs::read(&path)?)?)?;
    let settings_state = root.join(".si-extension-settings.json");
    let mut previous_settings: BTreeMap<String, String> = fs::read(&settings_state)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let settings = manifest
        .settings
        .into_iter()
        .map(|(key, value)| (key, expand_root(value, &root)))
        .collect();
    let conflicts = config::reconcile(&mut current, &mut previous_settings, &settings)?;
    if !conflicts.is_empty() {
        eprintln!(
            "Preserved user-edited extension settings: {}",
            conflicts.join(", ")
        );
    }
    fs::write(path, serde_json::to_vec_pretty(&current)?)?;
    fs::write(
        settings_state,
        serde_json::to_vec_pretty(&previous_settings)?,
    )?;
    let _ = nas.logout();
    Ok(())
}

fn expand_root(value: serde_json::Value, root: &Path) -> serde_json::Value {
    match value {
        serde_json::Value::String(text) => {
            serde_json::Value::String(text.replace("{root}", &root.to_string_lossy()))
        }
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .into_iter()
                .map(|item| expand_root(item, root))
                .collect(),
        ),
        serde_json::Value::Object(items) => serde_json::Value::Object(
            items
                .into_iter()
                .map(|(key, item)| (key, expand_root(item, root)))
                .collect(),
        ),
        other => other,
    }
}

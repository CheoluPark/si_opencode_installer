use crate::config;
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
    let code = if input.code_pat.trim().is_empty() {
        None
    } else {
        Some(input.code_pat.as_str())
    };

    let profile = PathBuf::from(env::var("USERPROFILE")?);
    let username = env::var("USERNAME")?;
    let root = profile.join(".config/opencode");
    let bundle = staging.join(".config/opencode");
    if !bundle.is_dir() {
        return Err("Staged release has no .config/opencode bundle".into());
    }
    if root.exists() && !overwrite_confirmed {
        return Err("Existing OpenCode config requires explicit overwrite confirmation".into());
    }
    let root_text = root.to_string_lossy();
    let replacements = BTreeMap::from([
        ("{{SI_LLM_API_KEY}}", input.api_key.as_str()),
        ("{{SI_WIKI_PAT}}", input.wiki_pat.as_str()),
        ("{{SI_WORK_PAT}}", input.work_pat.as_str()),
        ("{{SI_CODE_PAT}}", code.unwrap_or("")),
        ("{{SI_USER}}", username.as_str()),
        ("{{SI_OPENCODE_ROOT}}", root_text.as_ref()),
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
            validate_v2(&value, &generated.path().join("opencode.schema.json"))?;
        }
    }
    if !config_found {
        return Err("Staged config bundle has no opencode.json or opencode.jsonc".into());
    }
    deploy_bundle(generated.path(), &root, overwrite_confirmed)?;
    let manifest: serde_json::Value = serde_json::from_slice(&fs::read(staging.join("release.json"))?)?;
    let version = manifest["version"].as_str().ok_or("Staged release has no version")?;
    let state_dir = PathBuf::from(env::var("LOCALAPPDATA")?).join("SI OpenCode");
    fs::create_dir_all(&state_dir)?;
    fs::write(state_dir.join("installed-release.json"), serde_json::json!({"version":version}).to_string())?;
    Ok(())
}

/// Applies a new release without re-rendering the template. The existing
/// opencode.jsonc contains the user's API key and PATs and must remain intact.
pub fn apply_update(staging: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let root = PathBuf::from(env::var("USERPROFILE")?).join(".config/opencode");
    let bundle = staging.join(".config/opencode");
    if !root.is_dir() || !bundle.is_dir() {
        return Err("OpenCode config bundle is unavailable for update".into());
    }
    for name in ["mcp", "plugin", "skills", "lsp"] {
        replace_managed_directory(&bundle.join(name), &root.join(name))?;
    }
    let manifest: serde_json::Value = serde_json::from_slice(&fs::read(staging.join("release.json"))?)?;
    let version = manifest["version"].as_str().ok_or("Staged release has no version")?.to_owned();
    let state_dir = PathBuf::from(env::var("LOCALAPPDATA")?).join("SI OpenCode");
    fs::create_dir_all(&state_dir)?;
    fs::write(state_dir.join("installed-release.json"), serde_json::json!({"version":version}).to_string())?;
    Ok(version)
}

fn replace_managed_directory(source: &Path, target: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let parent = target.parent().ok_or("Invalid managed directory")?;
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let staged = parent.join(format!(".si-update-new-{}-{stamp}", target.file_name().unwrap().to_string_lossy()));
    let backup = parent.join(format!(".si-update-old-{}-{stamp}", target.file_name().unwrap().to_string_lossy()));
    if source.is_dir() { copy_tree(source, &staged)?; }
    if target.exists() { fs::rename(target, &backup)?; }
    let result = if source.is_dir() { fs::rename(&staged, target) } else { Ok(()) };
    if let Err(error) = result {
        let _ = fs::remove_dir_all(&staged);
        if backup.exists() { let _ = fs::rename(&backup, target); }
        return Err(error.into());
    }
    if backup.exists() { fs::remove_dir_all(backup)?; }
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
fn validate_v2(value: &serde_json::Value, schema_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let schema: serde_json::Value = serde_json::from_slice(&fs::read(schema_path)?)?;
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
    fn clean_install_config_bundle_is_rendered_validated_and_deployed_atomically() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("release/.config/opencode");
        let generated = temp.path().join("generated");
        let root = temp.path().join("profile/.config/opencode");
        fs::create_dir_all(&source).unwrap();
        fs::write(
            source.join("opencode.jsonc"),
            r#"{"$schema":"https://opencode.ai/config.json","model":"{{API_KEY}}", // SI:CODE_PAT:BEGIN
"code":"{{CODE_PAT}}",
// SI:CODE_PAT:END
"home":"{{USERPROFILE}}","base":"{{LLM_BASE_URL}}"}"#,
        )
        .unwrap();
        let vars = BTreeMap::from([
            ("{{API_KEY}}", "si/GPT"),
            ("{{WIKI_PAT}}", "wiki"),
            ("{{WORK_PAT}}", "work"),
            ("{{CODE_PAT}}", ""),
            ("{{USERPROFILE}}", "C:\\Users\\clean"),
            ("{{LLM_BASE_URL}}", "https://llm.example/v1"),
        ]);

        render_bundle(&source, &generated, &vars, false).unwrap();
        let rendered = fs::read_to_string(generated.join("opencode.jsonc")).unwrap();
        let config = config::parse_jsonc(&rendered).unwrap();
        validate_v2(&config, Path::new("config/opencode-v2.schema.json")).unwrap();
        assert_eq!(config["model"], "si/GPT");
        assert_eq!(config["home"], "C:\\Users\\clean");
        assert!(!rendered.contains("{{"));

        deploy_bundle(&generated, &root, false).unwrap();
        assert_eq!(
            fs::read_to_string(root.join("opencode.jsonc")).unwrap(),
            rendered
        );
        fs::write(root.join("user-data"), "keep").unwrap();
        assert!(deploy_bundle(&generated, &root, false).is_err());
        assert_eq!(fs::read_to_string(root.join("user-data")).unwrap(), "keep");

        fs::write(source.join("opencode.jsonc"), r#"{"pat":"{{BROKEN}}"}"#).unwrap();
        assert!(render_bundle(&source, &generated, &vars, false).is_err());
        assert_eq!(fs::read_to_string(root.join("user-data")).unwrap(), "keep");
    }

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
        validate_v2(&valid, Path::new("config/opencode-v2.schema.json")).unwrap();
        let invalid = serde_json::json!({"model":3});
        assert!(validate_v2(&invalid, Path::new("config/opencode-v2.schema.json")).is_err());
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

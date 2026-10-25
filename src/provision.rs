use crate::{config, load_state, save_state};
use serde::Deserialize;
use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

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
    if [input.api_key.as_str(), &input.wiki_pat, &input.work_pat]
        .iter()
        .any(|v| v.trim().is_empty())
    {
        return Err("LLM key, wiki PAT and work PAT are mandatory".into());
    }
    let skip = std::env::var_os("SI_SKIP_VALIDATE").is_some();
    let (base_url, code) = if skip {
        let code = if input.code_pat.trim().is_empty() {
            None
        } else {
            Some(input.code_pat.as_str())
        };
        ("https://ai.satreci.com:8443/v1".to_string(), code)
    } else {
        let chat_url = crate::validate::llm(&input.api_key)?;
        let base_url = chat_url
            .strip_suffix("/chat/completions")
            .ok_or("Unexpected LLM endpoint")?
            .to_string();
        crate::validate::mcp("https://dxdev.satreci.com/mcpwiki/mcp", &input.wiki_pat)?;
        crate::validate::mcp("https://dxdev.satreci.com/mcpwork/mcp", &input.work_pat)?;
        let mut code = if input.code_pat.trim().is_empty() {
            None
        } else {
            Some(input.code_pat.as_str())
        };
        if let Some(pat) = code {
            if let Err(error) = crate::validate::mcp("https://dxdev.satreci.com/mcpcode/mcp", pat) {
                if !confirm_skip_code(&error) {
                    return Err(error.into());
                }
                code = None;
            }
        }
        (base_url, code)
    };
    let root = std::path::PathBuf::from(env::var("USERPROFILE")?).join(".config/opencode");
    fs::create_dir_all(&root)?;
    let mut state = load_state(&root)?;
    let config_file = config_path(&root);
    let backup = root.join(".si-install-config-backup.jsonc");
    if backup.exists() {
        return Err("Previous install left a config backup; inspect it before retrying".into());
    }
    let previous = match fs::read(&config_file) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    let mut current = if let Some(bytes) = &previous {
        config::parse_jsonc(std::str::from_utf8(bytes)?)?
    } else {
        serde_json::json!({"$schema":"https://opencode.ai/config.json"})
    };
    let mcp_dir = root.join("mcp");
    let skills_dir = root.join("skills");
    let plugin_dir = root.join("plugin");
    let lsp_dir = root.join("lsp");
    fs::create_dir_all(&mcp_dir)?;
    fs::create_dir_all(&skills_dir)?;
    fs::create_dir_all(&plugin_dir)?;
    fs::create_dir_all(&lsp_dir)?;
    let values = config::initial(
        &input.api_key,
        &input.wiki_pat,
        &input.work_pat,
        code,
        &base_url,
        &root,
    );
    config::install_values(&mut current, &mut state.config, &values)?;
    let tmp = root.join(".si-install-new.jsonc");
    fs::write(&tmp, serde_json::to_vec_pretty(&current)?)?;
    if let Some(bytes) = &previous {
        fs::write(&backup, bytes)?;
    }
    if let Err(error) = (|| -> io::Result<()> {
        fs::rename(&tmp, &config_file).or_else(|_| {
            fs::remove_file(&config_file)?;
            fs::rename(&tmp, &config_file)
        })?;
        save_state(&root, &state)
    })() {
        match previous {
            Some(bytes) => fs::write(config_file, bytes)?,
            None => {
                let _ = fs::remove_file(config_file);
            }
        }
        if backup.exists() {
            fs::remove_file(backup)?;
        }
        return Err(error.into());
    }
    if backup.exists() {
        fs::remove_file(backup)?;
    }
    Ok(())
}

/// OpenCode reads `opencode.json` or `opencode.jsonc`; update the one that exists
/// instead of creating a second file, which would make the two compete.
pub fn config_path(root: &Path) -> PathBuf {
    let jsonc = root.join("opencode.jsonc");
    let json = root.join("opencode.json");
    if jsonc.exists() {
        jsonc
    } else if json.exists() {
        json
    } else {
        jsonc
    }
}

pub fn uninstall_managed() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::PathBuf::from(env::var("USERPROFILE")?).join(".config/opencode");
    if !root.exists() {
        return Ok(());
    }
    let mut state = load_state(&root)?;
    let empty = crate::Release {
        schema: 1,
        version: state.version,
        files: std::collections::BTreeMap::new(),
    };
    let staging = tempfile::tempdir_in(&root)?;
    let conflicts = crate::apply(&root, staging.path(), &mut state, &empty)?;
    let config_file = config_path(&root);
    if config_file.exists() {
        let previous = fs::read(&config_file)?;
        let mut current = config::parse_jsonc(std::str::from_utf8(&previous)?)?;
        state.config.retain(|key, _| {
            key.starts_with("mcp.servers.")
                && !matches!(
                    key.as_str(),
                    "mcp.servers.wiki" | "mcp.servers.work" | "mcp.servers.code"
                )
        });
        let protected = config::reconcile(
            &mut current,
            &mut state.config,
            &std::collections::BTreeMap::new(),
        )?;
        if !protected.is_empty() {
            eprintln!("Preserved user-edited settings: {}", protected.join(", "));
        }
        if let Some(plugin) = state.context_mode_plugin.take() {
            config::remove_plugin(&mut current, &plugin);
        }
        fs::write(&config_file, serde_json::to_vec_pretty(&current)?)?;
        if let Err(error) = save_state(&root, &state) {
            fs::write(config_file, previous)?;
            return Err(error.into());
        }
    }
    if !conflicts.is_empty() {
        eprintln!("Preserved user-edited files: {}", conflicts.join(", "));
    }
    Ok(())
}

pub fn install_managed(app: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let encoded =
        option_env!("SI_PUBLISHER_PUBLIC_KEY").ok_or("Publisher public key missing at build")?;
    let key = STANDARD.decode(encoded)?;
    let bytes = fs::read(app.join("manifest.json"))?;
    let signature = fs::read_to_string(app.join("manifest.sig"))?;
    let release = crate::verify_manifest(&bytes, &signature, &key)?;
    let root = std::path::PathBuf::from(env::var("USERPROFILE")?).join(".config/opencode");
    fs::create_dir_all(&root)?;
    let mut state = load_state(&root)?;
    let conflicts = crate::apply(&root, &app.join("managed"), &mut state, &release)?;
    if !conflicts.is_empty() {
        return Err(format!(
            "Existing user-owned files conflict: {}",
            conflicts.join(", ")
        )
        .into());
    }
    let config_file = config_path(&root);
    let mut current = config::parse_jsonc(std::str::from_utf8(&fs::read(&config_file)?)?)?;
    for plugin in [
        root.join("plugin/context-mode/node_modules/context-mode"),
        root.join("plugin/planning-with-files/node_modules/opencode-planning-with-files"),
        root.join("plugin/ponytail/node_modules/@dietrichgebert/ponytail"),
    ] {
        if plugin.exists() {
            let plugin = plugin.to_string_lossy().into_owned();
            config::add_plugin(&mut current, plugin.clone())?;
            state.context_mode_plugin = Some(plugin);
        }
    }
    fs::write(&config_file, serde_json::to_vec_pretty(&current)?)?;
    save_state(&root, &state)?;
    Ok(())
}

#[cfg(windows)]
fn confirm_skip_code(reason: &str) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, IDYES, MB_ICONWARNING, MB_YESNO,
    };
    let text: Vec<_> =
        format!("Bitbucket MCP validation failed ({reason}). Continue WITHOUT code MCP?")
            .encode_utf16()
            .chain([0])
            .collect();
    let caption: Vec<_> = "SI OpenCode Setup".encode_utf16().chain([0]).collect();
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            caption.as_ptr(),
            MB_ICONWARNING | MB_YESNO,
        ) == IDYES
    }
}

#[cfg(not(windows))]
fn confirm_skip_code(_reason: &str) -> bool {
    false
}

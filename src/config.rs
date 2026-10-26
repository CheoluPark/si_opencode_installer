use crate::sha256;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn parse_jsonc(input: &str) -> Result<Value, String> {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut quoted = false;
    let mut escape = false;
    while let Some(ch) = chars.next() {
        if quoted {
            output.push(ch);
            if ch == '"' && !escape {
                quoted = false;
            }
            if ch == '\\' {
                escape = !escape;
            } else {
                escape = false;
            }
            continue;
        }
        if ch == '"' {
            quoted = true;
            output.push(ch);
            continue;
        }
        if ch == '/' && chars.peek() == Some(&'/') {
            chars.next();
            for next in chars.by_ref() {
                if next == '\n' {
                    output.push('\n');
                    break;
                }
            }
            continue;
        }
        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut previous = '\0';
            let mut closed = false;
            for next in chars.by_ref() {
                if previous == '*' && next == '/' {
                    closed = true;
                    break;
                }
                previous = next;
            }
            if !closed {
                return Err("Unterminated JSONC comment".into());
            }
            output.push(' ');
            continue;
        }
        output.push(ch);
    }
    if quoted {
        return Err("Unterminated JSONC string".into());
    }
    let mut cleaned = String::with_capacity(output.len());
    let mut chars = output.chars().peekable();
    let mut quoted = false;
    let mut escape = false;
    while let Some(ch) = chars.next() {
        if quoted {
            cleaned.push(ch);
            if ch == '"' && !escape {
                quoted = false;
            }
            if ch == '\\' {
                escape = !escape;
            } else {
                escape = false;
            }
        } else if ch == '"' {
            quoted = true;
            cleaned.push(ch);
        } else if ch == ','
            && chars
                .clone()
                .find(|c| !c.is_whitespace())
                .is_some_and(|c| c == '}' || c == ']')
        {
            cleaned.push(' ');
        } else {
            cleaned.push(ch);
        }
    }
    serde_json::from_str(&cleaned).map_err(|error| error.to_string())
}

fn at<'a>(config: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.')
        .try_fold(config, |value, key| value.get(key))
}

fn set(config: &mut Value, path: &str, value: Option<Value>) -> Result<(), String> {
    let mut keys = path.split('.').peekable();
    let mut cursor = config;
    while let Some(key) = keys.next() {
        let map = cursor
            .as_object_mut()
            .ok_or("Expected object in OpenCode config")?;
        if keys.peek().is_none() {
            if let Some(value) = value {
                map.insert(key.into(), value);
            } else {
                map.remove(key);
            }
            return Ok(());
        }
        cursor = map.entry(key).or_insert_with(|| json!({}));
    }
    Err("Empty configuration path".into())
}

pub fn remove(config: &mut Value, path: &str) -> Result<(), String> {
    set(config, path, None)
}

fn fingerprint(value: &Value) -> String {
    sha256(serde_json::to_vec(value).unwrap().as_slice())
}

pub fn install_values(
    current: &mut Value,
    state: &mut BTreeMap<String, String>,
    values: &BTreeMap<String, Value>,
) -> Result<(), String> {
    for (key, value) in values {
        set(current, key, Some(value.clone()))?;
        state.insert(key.clone(), fingerprint(value));
    }
    Ok(())
}

pub fn add_plugin(config: &mut Value, plugin: String) -> Result<(), String> {
    let object = config
        .as_object_mut()
        .ok_or("OpenCode config must be an object")?;
    let plugins = object.entry("plugins").or_insert_with(|| json!([]));
    let plugins = plugins
        .as_array_mut()
        .ok_or("OpenCode plugins must be an array")?;
    if !plugins.iter().any(|value| value.as_str() == Some(&plugin)) {
        plugins.push(Value::String(plugin));
    }
    Ok(())
}

pub fn remove_plugin(config: &mut Value, plugin: &str) {
    if let Some(plugins) = config.get_mut("plugins").and_then(Value::as_array_mut) {
        plugins.retain(|value| value.as_str() != Some(plugin));
    }
}

/// Protects user-edited values; old hashes do not duplicate PATs in installed state.
pub fn reconcile(
    current: &mut Value,
    old: &mut BTreeMap<String, String>,
    wanted: &BTreeMap<String, Value>,
) -> Result<Vec<String>, String> {
    let mut conflicts = Vec::new();
    let keys: std::collections::BTreeSet<_> = old.keys().chain(wanted.keys()).cloned().collect();
    for key in keys {
        let existing = at(current, &key);
        if let Some(previous) = old.get(&key) {
            if existing.map(fingerprint).as_ref() != Some(previous) {
                conflicts.push(key);
                continue;
            }
        } else if existing.is_some() {
            conflicts.push(key);
            continue;
        }
        set(current, &key, wanted.get(&key).cloned())?;
        if let Some(value) = wanted.get(&key) {
            old.insert(key, fingerprint(value));
        } else {
            old.remove(&key);
        }
    }
    Ok(conflicts)
}

pub fn initial(
    api_key: &str,
    wiki: &str,
    work: &str,
    code: Option<&str>,
    base_url: &str,
    _root: &std::path::Path,
) -> BTreeMap<String, Value> {
    let mut result = BTreeMap::new();
    result.insert("model".into(), json!("si/GPT"));
    result.insert("providers.si".into(), json!({"name":"SI LLM", "package":"@opencode/ai/providers/openai-compatible", "settings":{"baseURL":base_url, "apiKey":api_key}, "models": {
        "GPT":{"modelID":"GPT", "capabilities":{"tools":true, "input":["text","image"], "output":["text"]}},
        "GPT2":{"modelID":"GPT2", "capabilities":{"input":["text","image"], "output":["text"]}}
    }}));
    for (name, url, pat) in [
        ("wiki", "https://dxdev.satreci.com/mcpwiki/mcp", wiki),
        ("work", "https://dxdev.satreci.com/mcpwork/mcp", work),
    ] {
        result.insert(format!("mcp.servers.{name}"), json!({"type":"remote","url":url,"oauth":false,"headers":{"Authorization":format!("Bearer {pat}")}}));
    }
    if let Some(pat) = code {
        result.insert("mcp.servers.code".into(), json!({"type":"remote","url":"https://dxdev.satreci.com/mcpcode/mcp","oauth":false,"headers":{"Authorization":format!("Bearer {pat}")}}));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsonc_preserves_strings_and_user_values() {
        let config =
            parse_jsonc("{ // own\n\"url\":\"https://host/a//b\",/*note*/\"items\":[1,],}")
                .unwrap();
        assert_eq!(config["url"], "https://host/a//b");
        assert_eq!(config["items"], json!([1]));
        assert!(parse_jsonc("{/* unfinished").is_err());
    }

    #[test]
    fn optional_code_pat_adds_only_its_mcp_server_when_present() {
        let without_code = initial("api", "wiki", "work", None, "https://ai.example/v1", std::path::Path::new("C:/Users/test"));
        assert!(!without_code.keys().any(|key| key == "mcp.servers.code"));
        let with_code = initial("api", "wiki", "work", Some("code-token"), "https://ai.example/v1", std::path::Path::new("C:/Users/test"));
        assert_eq!(with_code["mcp.servers.code"]["headers"]["Authorization"], "Bearer code-token");
    }

    #[test]
    fn user_edits_win_and_pat_not_copied_to_state() {
        let mut config = json!({"mcp":{"servers":{"wiki":{"key":"original"}}},"personal":7});
        let mut old = BTreeMap::from([(
            "mcp.servers.wiki".into(),
            fingerprint(&json!({"key":"original"})),
        )]);
        let wanted = BTreeMap::from([("mcp.servers.wiki".into(), json!({"key":"new"}))]);
        reconcile(&mut config, &mut old, &wanted).unwrap();
        assert_eq!(config["mcp"]["servers"]["wiki"]["key"], "new");
        assert!(!format!("{old:?}").contains("new"));
        config["mcp"]["servers"]["wiki"]["key"] = json!("personal");
        assert_eq!(
            reconcile(&mut config, &mut old, &BTreeMap::new()).unwrap(),
            ["mcp.servers.wiki"]
        );
        assert_eq!(config["mcp"]["servers"]["wiki"]["key"], "personal");
        assert_eq!(config["personal"], 7);
    }

    #[test]
    fn local_plugin_preserves_existing_plugins() {
        let mut config = json!({"plugins":["user-plugin"]});
        add_plugin(&mut config, "C:\\context-mode".into()).unwrap();
        add_plugin(&mut config, "C:\\context-mode".into()).unwrap();
        assert_eq!(
            config["plugins"],
            json!(["user-plugin", "C:\\context-mode"])
        );
        remove_plugin(&mut config, "C:\\context-mode");
        assert_eq!(config["plugins"], json!(["user-plugin"]));
    }
}

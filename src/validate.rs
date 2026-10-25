use reqwest::{
    blocking::{Client, Response},
    header::{ACCEPT, CONTENT_TYPE},
};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader},
    time::Duration,
};

fn client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())
}

pub fn llm(api_key: &str) -> Result<String, String> {
    for chat_url in [
        "https://ai.satreci.com:8443/v1/chat/completions",
        "https://ai.satreci.com:8443/chat/completions",
    ] {
        if let Some(()) = probe_llm(api_key, chat_url)? {
            return Ok(chat_url.into());
        }
    }
    Err(
        "Neither standard OpenAI-compatible chat path is available on the internal LLM server"
            .into(),
    )
}

fn probe_llm(api_key: &str, chat_url: &str) -> Result<Option<()>, String> {
    let response = client()?.post(chat_url).bearer_auth(api_key)
        .json(&json!({"model":"GPT","messages":[{"role":"user","content":"Reply with OK."}],"max_tokens":8}))
        .send().map_err(|e| e.to_string())?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let response = response.error_for_status().map_err(|e| e.to_string())?;
    let result: Value = response.json().map_err(|e| e.to_string())?;
    if result["choices"][0]["message"]["content"].is_string() {
        let probe = client()?.post(chat_url).bearer_auth(api_key)
            .json(&json!({"model":"GPT","messages":[{"role":"user","content":"Call the probe tool once."}],"max_tokens":64,
                "tools":[{"type":"function","function":{"name":"probe","description":"Installation tool-calling test","parameters":{"type":"object","properties":{}}}}],
                "tool_choice":"required"}))
            .send().map_err(|e| e.to_string())?.error_for_status().map_err(|e| e.to_string())?;
        let tools: Value = probe.json().map_err(|e| e.to_string())?;
        if tools["choices"][0]["message"]["tool_calls"]
            .as_array()
            .is_some_and(|calls| !calls.is_empty())
        {
            Ok(Some(()))
        } else {
            Err("GPT cannot call MCP tools on this server".into())
        }
    } else {
        Err("LLM returned no text response".into())
    }
}

pub fn mcp(url: &str, pat: &str) -> Result<(), String> {
    if !matches!(
        url,
        "https://dxdev.satreci.com/mcpwiki/mcp"
            | "https://dxdev.satreci.com/mcpwork/mcp"
            | "https://dxdev.satreci.com/mcpcode/mcp"
    ) {
        return Err("Untrusted MCP URL".into());
    }
    let client = client()?;
    let init = client.post(url).bearer_auth(pat)
        .header(ACCEPT, "application/json, text/event-stream")
        .json(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"si-opencode-installer","version":"1.0"}}}))
        .send().map_err(|e| e.to_string())?.error_for_status().map_err(|e| e.to_string())?;
    let session = init
        .headers()
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let initialized = rpc_result(init, 1)?;
    let protocol = initialized["result"]["protocolVersion"]
        .as_str()
        .ok_or("No negotiated MCP protocol version")?;
    let mut notice = client
        .post(url)
        .bearer_auth(pat)
        .header(ACCEPT, "application/json, text/event-stream")
        .header("mcp-protocol-version", protocol)
        .json(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    if let Some(ref session) = session {
        notice = notice.header("mcp-session-id", session);
    }
    notice
        .send()
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let mut request = client
        .post(url)
        .bearer_auth(pat)
        .header(ACCEPT, "application/json, text/event-stream")
        .header("mcp-protocol-version", protocol)
        .json(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}));
    if let Some(session) = session {
        request = request.header("mcp-session-id", session);
    }
    let listed = request
        .send()
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let result = rpc_result(listed, 2)?;
    if result["result"]["tools"].as_array().is_some() {
        Ok(())
    } else {
        Err("MCP tools/list returned no tool list".into())
    }
}

pub fn local_kb() -> Result<(), String> {
    let url = "http://127.0.0.1:8080/mcp/doc-search";
    let client = client()?;
    let request = client.post(url).header(ACCEPT, "application/json, text/event-stream")
        .json(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"si-opencode-installer","version":"1.0"}}}))
        .send().map_err(|e| e.to_string())?.error_for_status().map_err(|e| e.to_string())?;
    let session = request
        .headers()
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let initialized = rpc_result(request, 1)?;
    let protocol = initialized["result"]["protocolVersion"]
        .as_str()
        .ok_or("No KB protocol version")?;
    let mut notice = client
        .post(url)
        .header(ACCEPT, "application/json, text/event-stream")
        .header("mcp-protocol-version", protocol)
        .json(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    if let Some(ref sid) = session {
        notice = notice.header("mcp-session-id", sid);
    }
    notice
        .send()
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let mut list = client
        .post(url)
        .header(ACCEPT, "application/json, text/event-stream")
        .header("mcp-protocol-version", protocol)
        .json(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}));
    if let Some(ref sid) = session {
        list = list.header("mcp-session-id", sid);
    }
    let tools = rpc_result(
        list.send()
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?,
        2,
    )?;
    let search = tools["result"]["tools"]
        .as_array()
        .and_then(|array| {
            array
                .iter()
                .find(|tool| tool["name"] == "vivado_doc_search")
        })
        .ok_or("KB tool vivado_doc_search not found")?;
    let properties = search["inputSchema"]["properties"]
        .as_object()
        .ok_or("KB search input schema missing")?;
    let key = ["query", "question", "search_query"]
        .iter()
        .find(|name| properties.contains_key(**name))
        .ok_or("KB query parameter unknown")?;
    let mut arguments = serde_json::Map::new();
    arguments.insert(
        (*key).into(),
        json!("Search UG904 for incremental compile."),
    );
    let mut call = client.post(url).header(ACCEPT, "application/json, text/event-stream")
        .header("mcp-protocol-version", protocol)
        .json(&json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"vivado_doc_search","arguments":arguments}}));
    if let Some(ref sid) = session {
        call = call.header("mcp-session-id", sid);
    }
    let result = rpc_result(
        call.send()
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?,
        3,
    )?;
    if result["result"]["isError"] == true
        || !result["result"]["content"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    {
        return Err("KB returned no documentation".into());
    }
    Ok(())
}

fn rpc_result(response: Response, id: u64) -> Result<Value, String> {
    let sse = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.contains("text/event-stream"));
    if !sse {
        let value: Value = response.json().map_err(|e| e.to_string())?;
        return check_id(value, id);
    }
    for line in BufReader::new(response).lines().take(256) {
        let line = line.map_err(|e| e.to_string())?;
        if let Some(data) = line.strip_prefix("data:") {
            let value: Value = serde_json::from_str(data.trim()).map_err(|e| e.to_string())?;
            if value["id"] == id {
                return check_id(value, id);
            }
        }
    }
    Err("MCP stream yielded no matching result".into())
}

fn check_id(value: Value, id: u64) -> Result<Value, String> {
    if value["id"] != id || !value["result"].is_object() {
        Err("MCP call returned an error or unrelated response".into())
    } else {
        Ok(value)
    }
}

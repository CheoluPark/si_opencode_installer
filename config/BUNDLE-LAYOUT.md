# OpenCode extension bundle layout

Build the extension bundle outside Git, then extract it to:

```text
%USERPROFILE%\.config\opencode\
```

The archive root must contain these folders:

```text
mcp/
plugin/
skills/
lsp/
```

Required configured paths:

```text
lsp/node/node.exe
mcp/rea/node_modules/rea-agents/scripts/rea.mjs
mcp/drawio/node_modules/@next-ai-drawio/mcp-server/dist/index.js
plugin/context-mode/node_modules/context-mode
plugin/planning-with-files/node_modules/opencode-planning-with-files
plugin/ponytail/node_modules/@dietrichgebert/ponytail
```

`skills/` and `lsp/` are included for OpenCode-compatible discovery and other local tools. OpenCode does not currently execute LSP servers itself.

## Later extension updates from FileStation

Place an `extensions-manifest.json` next to the extension files on `file.satreci.com`. The local config is `%LOCALAPPDATA%\SI OpenCode\extensions.json`; edit its `manifest_path` if the FileStation folder differs.

```json
{
  "files": {
    "mcp/example/node_modules/example-mcp/index.js": {
      "sha256": "<lowercase sha256>",
      "bytes": 1234
    }
  },
  "settings": {
    "mcp.servers.example": {
      "type": "local",
      "command": [
        "{root}/lsp/node/node.exe",
        "{root}/mcp/example/node_modules/example-mcp/index.js"
      ],
      "disabled": true
    },
    "lsp.example-language": {
      "command": ["{root}/lsp/example/example-ls.exe", "--stdio"],
      "extensions": [".example"],
      "disabled": true
    }
  }
}
```

`settings` is a map of `opencode.jsonc` paths to the exact JSON values you want. The updater only replaces `{root}` with the deployment PC's `%USERPROFILE%\.config\opencode` path. It does not infer a command, arguments, plugin format, or LSP options.

Run `si_opencode_updater.exe --sync-extensions`. It downloads changed files, removes files absent from the new manifest only when they still match its recorded hash, and applies the declared settings. Put `"disabled": true` in every MCP/LSP setting that must start disabled.

# External release assets

This repository contains the installer source only. Do not commit the files below.

## Build machine prerequisites

- Windows x64
- Rust/MSVC toolchain, with this repository's `vendor/` directory available
- Inno Setup 6
- A 32-byte Ed25519 publisher public key file
- A publisher-only Ed25519 PKCS#8 private key for release publishing

Build the installer:

```powershell
.\build.ps1 -PublicKey C:\secure\publisher-ed25519.pub
```

This writes `out\si_opencode_installer.exe`. The public key is compiled into the updater. Never commit the private key.

## FileStation release layout

Prepare a temporary `staging/` directory outside Git:

```text
staging/
├── .config/
│   └── opencode/
│       ├── opencode.template.jsonc
│       ├── opencode.schema.json
│       ├── mcp/
│       ├── plugin/
│       ├── skills/
│       └── lsp/
└── tools/
    ├── tools-manifest.json
    ├── opencode_desktop_win64.exe
    └── optional tool installers
```

`mcp/`, `plugin/`, `skills/`, and `lsp/` contain every dependency required in the closed network. They are copied to `%USERPROFILE%\.config\opencode` during initial install and updated from later releases.

## Template variables

Only these markers are replaced by the installer:

```text
{{SI_LLM_API_KEY}}
{{SI_WIKI_PAT}}
{{SI_WORK_PAT}}
{{SI_CODE_PAT}}
{{SI_USER}}
{{SI_OPENCODE_ROOT}}
```

The optional Code MCP block must be delimited by these JSONC comments. It is removed if Code PAT is blank.

```jsonc
// SI:CODE_PAT:BEGIN
// SI:CODE_PAT:END
```

All other template content is release-author controlled. `opencode.schema.json` must be the OpenCode v2 schema approved for the bundled template.

## Tool manifest

`tools/tools-manifest.json` must declare `desktop` and may declare `msvc`, `rust`, `python`, `node`, and `wsl` under `tools`.

```json
{
  "desktop": {
    "installer": "tools/opencode_desktop_win64.exe",
    "sha256": "<64 lowercase hex characters>",
    "minimum_version": "1.0.0"
  },
  "tools": {
    "python": {
      "name": "Python 3.15",
      "installer": "tools/python-3.15.0-amd64.exe",
      "sha256": "<64 lowercase hex characters>",
      "minimum_version": "3.15.0",
      "detect_path": "Programs\\Python\\Python315\\python.exe",
      "install_args": "/quiet InstallAllUsers=0 PrependPath=1",
      "scope": "user"
    }
  }
}
```

Every EXE/MSI must have a valid Authenticode signature. The installer verifies its manifest hash before execution.

## Publish a release

```powershell
cargo run --offline --bin release -- keygen C:\secure\publisher.pk8 C:\secure\publisher.pub
cargo run --offline --bin release -- build C:\release\staging 2026.10.28.1 C:\secure\publisher.pk8
```

The builder creates `release.json` and `release.sig`. Upload the fully prepared folder to FileStation `staging/`, verify it, then promote it to `current/`. Keep only `current/` and temporary `staging/` to control storage use.

## Client-side files

Distribute only:

```text
si_opencode_installer.exe
```

The installer downloads all release content from FileStation. It stores the release-path setting at:

```text
%LOCALAPPDATA%\SI OpenCode\release.json
```

The default setting is supplied by `config/release.example.json`. NAS credentials are stored in Windows Credential Manager.

## Required corporate test

On a clean Windows account, verify FileStation login, release download, Desktop install, selected tools, `.config` replacement backup, generated `opencode.jsonc`, scheduled update task, approved update, and OpenCode restart.

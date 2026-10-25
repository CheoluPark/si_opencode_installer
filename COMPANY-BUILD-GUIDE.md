# Company build guide

This repository deliberately does **not** contain large release assets, private keys, or the reviewed direct-copy payload. Prepare them inside the corporate network before building.

## 1. Required tools

Install these on the Windows x64 build PC:

- Rust/MSVC toolchain
- Inno Setup 6 (`%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe`)
- Node.js with `npm` (needed only to test Nexus installation; the build itself does not download npm packages)
- Git

The repository contains `.cargo/config.toml` and `vendor/` for offline Rust builds.

## 2. Files excluded from Git

Create the following files under `payload/`. They are ignored by Git and must be obtained separately.

```text
payload/
├── opencode_desktop_win64.exe
└── managed/
    ├── mcp/                 # only MCP files that cannot be installed from npm
    ├── plugin/              # only plugins that cannot be installed from npm
    ├── skills/              # only skills that cannot be installed from npm
    └── lsp/
        └── node/
            └── node.exe     # bundled Node runtime for local MCP commands
```

`payload/managed/` is copied unchanged to `%USERPROFILE%\.config\opencode\` through `managed-runtime.tar.gz`.

Do **not** put an npm-installable package into `payload/managed/`. Put it in Nexus and add it to the package list described below.

## 3. Required local signing keys

Keep the key pair outside the repository. The current expected location is:

```text
%LOCALAPPDATA%\SI OpenCode\release-keys\publisher-ed25519.pk8
%LOCALAPPDATA%\SI OpenCode\release-keys\publisher-ed25519.pub
```

Generate a new pair if needed:

```powershell
cargo run --offline --bin release -- keygen <private-key.pk8> <public-key.pub>
```

Never commit or distribute `publisher-ed25519.pk8`.

## 4. Nexus configuration — required edit

Edit `config/managed-delivery.json` before building.

Replace this value:

```json
"npmRegistry": "https://NEXUS_NPM_REGISTRY_PLACEHOLDER/repository/npm-group/"
```

with the actual corporate Nexus npm-group URL, for example:

```json
"npmRegistry": "https://nexus.company.example/repository/npm-group/"
```

The installer intentionally stops if `PLACEHOLDER` is still present.

### Package entries

Every npm package has this shape:

```json
{
  "target": "mcp/example",
  "package": "@company/example-mcp",
  "version": "1.2.3"
}
```

- `target` is relative to `%USERPROFILE%\.config\opencode\`.
- Use only `mcp/`, `plugin/`, `skills/`, or `lsp/` as the first target folder.
- Pin an exact version. Do not use `latest`, `^`, or `~`.
- The installer runs `npm install --prefix <target> <package>@<version>`.

Current entries must exist in Nexus before the installer is tested:

- `@dietrichgebert/ponytail@5.3.2`
- `context-mode@1.0.169`
- `opencode-planning-with-files@1.2.0`
- `ui-ux-pro-max-cli@2.15.0`
- `rea-agents@6.7.0`
- `@next-ai-drawio/mcp-server@0.4.6`
- `pyright@1.1.414`
- `typescript@7.0.2`
- `typescript-language-server@6.0.2`

If one cannot be served by Nexus, remove its entry and place its reviewed files under the matching `payload/managed/{mcp,plugin,skills,lsp}/` folder instead.

## 5. Build

From the repository root:

```powershell
.\build.ps1 `
  -PrivateKey "$env:LOCALAPPDATA\SI OpenCode\release-keys\publisher-ed25519.pk8" `
  -PublicKey "$env:LOCALAPPDATA\SI OpenCode\release-keys\publisher-ed25519.pub" `
  -Version 1
```

Increase `-Version` for every release.

The build:

1. creates `payload/managed-runtime.tar.gz` from direct-copy files only;
2. signs its direct-copy file manifest;
3. builds `si_opencode_updater.exe`;
4. compiles the Inno installer;
5. copies the distribution files to `out/`.

## 6. Files to distribute

Keep these three files in the **same folder**:

```text
out/
├── si_opencode_installer.exe
├── opencode_desktop_win64.exe
└── managed-runtime.tar.gz
```

The installer EXE is intentionally small. It verifies that the two sidecars exist while installing.

## 7. First corporate-network test

Use a clean Windows test account.

1. Put the three distribution files in one folder.
2. Run `si_opencode_installer.exe`.
3. Enter a valid LLM API key, wiki PAT, and work PAT. The code PAT is optional.
4. Confirm that OpenCode Desktop starts.
5. Check `%USERPROFILE%\.config\opencode\opencode.jsonc`.
6. Confirm the required npm targets exist below `%USERPROFILE%\.config\opencode\`.
7. Open OpenCode and verify the `rea` and `drawio` MCP servers plus the installed plugins.

## 8. Current scope

This build does **not** include FPGA KB, WSL, Docker, NAS updates, scheduled tasks, release-notes UI, or the old bootstrap package. It only performs initial OpenCode installation and managed extension setup.

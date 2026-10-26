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

`payload/managed/` is copied unchanged into `opencode-config-bundle.tar.gz`. Extract that archive to `%USERPROFILE%\.config\opencode\` before running the installer.

Put every required dependency in this bundle. The closed-network npm proxy is not used.

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

## 4. Later extension updates

Use `config/extensions.example.json` to set the FileStation manifest path. See `config/BUNDLE-LAYOUT.md` for the manifest format. Running `si_opencode_updater.exe --sync-extensions` downloads only the listed extension files and registers each managed MCP as disabled.

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

1. creates `payload/opencode-config-bundle.tar.gz` from the full extension tree;
2. builds `si_opencode_updater.exe`;
3. compiles the Inno installer;
4. copies the distribution files to `out/`.

## 6. Files to distribute

Keep these three files in the **same folder**:

```text
out/
├── si_opencode_installer.exe
├── opencode_desktop_win64.exe
└── opencode-config-bundle.tar.gz
```

The installer EXE is intentionally small. Extract the bundle before installing; the installer only registers its paths in `opencode.jsonc`.

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

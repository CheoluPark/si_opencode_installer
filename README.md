# SI OpenCode installer

Windows installer for SI OpenCode.

## What it installs

1. Before setup starts, the bootstrap verifies and stages the signed current FileStation release.
2. Inno Setup asks for the internal LLM API key and wiki/work PATs.
3. `si_opencode_updater.exe --provision` validates those services and writes `%USERPROFILE%\.config\opencode\opencode.jsonc` without replacing unrelated user settings.
4. `opencode_desktop_win64.exe` installs OpenCode Desktop.

The updater supports `--provision`, `--sync-extensions`, `--fetch-current-release`, and `--launch`.

## Extension layout

`payload/managed/` contains every extension dependency. It may contain only these top-level folders:

```text
mcp/
plugin/
skills/
lsp/
```

Its contents are copied into `.config/opencode/` in the signed current release. The installer verifies and stages the release before starting setup.

## Build

Required assets in `payload/`:

```text
opencode_desktop_win64.exe
managed/lsp/node/node.exe
managed/{mcp,plugin,skills,lsp}/
```

Build from Windows with Rust/MSVC, Node/npm, and Inno Setup:

```powershell
.\build.ps1 -PublicKey <publisher.pub>
```

The release publisher stages `.config/opencode/` and `tools/` (including `tools/tools-manifest.json`) and signs them into `release.json` / `release.sig`. The generated config bundle is a build artifact; it is not a required file in the signed release.

The bootstrap installer still ships with the Desktop installer:

```text
out/si_opencode_installer.exe
out/opencode_desktop_win64.exe
```

Do not put a publisher private key in this repository or in a release folder.

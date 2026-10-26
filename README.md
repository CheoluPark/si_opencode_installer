# SI OpenCode installer

Windows installer for SI OpenCode.

## What it installs

1. Inno Setup asks for the internal LLM API key and wiki/work PATs.
2. `si_opencode_updater.exe --provision` validates those services and writes `%USERPROFILE%\.config\opencode\opencode.jsonc` without replacing unrelated user settings.
3. `opencode_desktop_win64.exe` installs OpenCode Desktop.
4. The user extracts the external extension bundle into `%USERPROFILE%\.config\opencode`; the installer registers those paths in `opencode.jsonc`.

The updater supports `--provision`, `--sync-extensions`, and `--launch`.

## Extension layout

`payload/managed/` contains every extension dependency. It may contain only these top-level folders:

```text
mcp/
plugin/
skills/
lsp/
```

Its contents are packaged unchanged into `opencode-config-bundle.tar.gz`. Extract it below `%USERPROFILE%\.config\opencode\` before running the installer.

## Build

Required assets in `payload/`:

```text
opencode_desktop_win64.exe
managed/lsp/node/node.exe
managed/{mcp,plugin,skills,lsp}/
```

Build from Windows with Rust/MSVC, Node/npm, and Inno Setup:

```powershell
.\build.ps1 -PrivateKey <publisher.pk8> -PublicKey <publisher.pub> -Version 12
```

Distribute these files together:

```text
out/si_opencode_installer.exe
out/opencode_desktop_win64.exe
out/opencode-config-bundle.tar.gz
```

Do not put a publisher private key in this repository or in a release folder.

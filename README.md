# SI OpenCode installer

Windows installer for SI OpenCode.

## What it installs

1. Inno Setup asks for the internal LLM API key and wiki/work PATs.
2. `si_opencode_updater.exe --provision` validates those services and writes `%USERPROFILE%\.config\opencode\opencode.jsonc` without replacing unrelated user settings.
3. `opencode_desktop_win64.exe` installs OpenCode Desktop.
4. `setup-runtime.ps1` extracts direct-copy extensions, installs Nexus npm packages, then applies the direct-copy files into `%USERPROFILE%\.config\opencode`.

The updater has only four commands: `--provision`, `--install-managed`, `--uninstall-managed`, and `--launch`.

## Extension layout

`payload/managed/` is only for files which cannot be installed from npm. It may contain only these top-level folders:

```text
mcp/
plugin/
skills/
lsp/
```

Its contents are packaged unchanged into `managed-runtime.tar.gz` and installed below `%USERPROFILE%\.config\opencode\`.

Npm-installable extensions belong in `config/managed-delivery.json`, not in `payload/managed/`. Set `npmRegistry` to the real Nexus registry before building. Every package is installed into its declared `target` below `%USERPROFILE%\.config\opencode\`.

## Build

Required assets in `payload/`:

```text
opencode_desktop_win64.exe
managed/lsp/node/node.exe
managed/{mcp,plugin,skills,lsp}/     # direct-copy files only
```

Build from Windows with Rust/MSVC, Node/npm, and Inno Setup:

```powershell
.\build.ps1 -PrivateKey <publisher.pk8> -PublicKey <publisher.pub> -Version 12
```

Distribute these files together:

```text
out/si_opencode_installer.exe
out/opencode_desktop_win64.exe
out/managed-runtime.tar.gz
```

Do not put a publisher private key in this repository or in a release folder.

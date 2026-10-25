# SI OpenCode Bootstrap

The bootstrap verifies `offline-payload/manifest.json` with its adjacent Ed25519 signature and public key, stages each managed artifact into `prepared-payload/managed`, runs `si_opencode_installer.exe`, then asks the installed signed updater to apply that prepared managed payload. The updater preserves its existing managed-file safety rules.

Run from the release folder:

```powershell
si-opencode-bootstrap
```

Use `--prepare-only` to validate and stage without launching the installer. The package uses an invocation-local npm configuration and never writes the user's `.npmrc`.

`offline-payload/` must mirror managed manifest paths and include `manifest.json`, `manifest.sig`, and `publisher-ed25519.pub`. A manifest entry defaults to `directCopy`; npm delivery is only used when explicitly declared and a required entry must have a direct-copy fallback.

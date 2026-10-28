# SI OpenCode installer

Windows installer and updater for a signed SI OpenCode release hosted on `file.satreci.com`.

The installer collects the LLM API key plus wiki/work PATs and optional Code PAT, downloads a signed FileStation release, installs OpenCode Desktop and selected tools, then deploys a rendered `%USERPROFILE%\.config\opencode` bundle.

The updater checks for a newer signed release at 10:00, 13:00, 16:00, 19:00, and 22:00. It asks before applying changes, retains existing API/PAT settings, replaces managed dependency folders atomically, and can restart OpenCode.

See [EXTERNAL-RELEASE-ASSETS.md](EXTERNAL-RELEASE-ASSETS.md) for all external files, release layout, manifest format, signing, publishing, and corporate validation.

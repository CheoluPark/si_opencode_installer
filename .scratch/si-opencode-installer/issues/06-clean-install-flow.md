# 06: Clean install flow

**What to build:** A clean Windows account can complete the full installer flow: input credentials, download and validate a release, install Desktop and selected tools, replace the config bundle, generate valid OpenCode configuration, and see the final warning/reboot summary.

**Blocked by:** 03: Template config deployment; 04: Install OpenCode Desktop; 05: Optional development tools.

**Status:** ready-for-agent

- [x] A fixture-backed integration test validates template rendering, JSONC/schema validation, deployment, and overwrite refusal without FileStation, credentials, Desktop, or tool installers.
- [x] Release signature/tampering, template-marker errors, and config overwrite refusal have offline automated coverage; installer UI cancellation and Windows side effects remain manual checks.

## Clean Windows manual validation

Use a disposable, newly-created **standard** Windows account on a clean x64 VM. No company credentials or personal profile data should be present. Use the current signed release and real authorized test credentials only for the live service steps.

1. Run the signed installer. Confirm the FileStation login and release download/verification complete; confirm required API key, wiki PAT, and work PAT are enforced while code PAT remains optional.
2. Cancel FileStation sign-in, then repeat and cancel the existing-config overwrite prompt (on a separate VM snapshot with a sentinel in `.config\opencode`). Confirm neither cancellation installs files nor changes the sentinel/config.
3. On a clean profile, finish installation with code PAT blank. Confirm Desktop and its CLI are installed, `opencode.exe` is placed under the app directory and added to the user PATH, and generated `.config\opencode` JSON parses and contains no template markers or code MCP server. Confirm secrets are not written into the generated config except the required service settings, and no plaintext installer input remains in `%TEMP%`.
4. Repeat using code PAT and confirm the optional code MCP server is present. Exercise overwrite on a profile with a sentinel config: accept, then verify the new config is active and the prior config exists in a backup; reject and verify the prior config is unchanged.
5. Select no optional tools and verify completion reports them as skipped. In another disposable VM, select an optional tool and verify successful installation/version detection; where practical use a machine-scope tool to verify UAC denial is reported as a warning rather than aborting the required setup.
6. Verify the final warning/reboot summary for optional-tool failures and reboot-required exit codes. Confirm no reboot is initiated unless required, and that the warning list identifies skipped/failed optional tools.
7. Verify installed OpenCode launches, then uninstall and confirm only installer-owned application files are removed; user config and its backup remain.

Do not simulate invalid signatures/hashes by modifying production artifacts on a real user profile. Those rejection paths are exercised offline by Rust tests; Windows UI, Authenticode, UAC, external service credentials, and actual installer behavior require the disposable-VM checks above.

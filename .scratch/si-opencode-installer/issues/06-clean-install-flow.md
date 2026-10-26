# 06: Clean install flow

**What to build:** A clean Windows account can complete the full installer flow: input credentials, download and validate a release, install Desktop and selected tools, replace the config bundle, generate valid OpenCode configuration, and see the final warning/reboot summary.

**Blocked by:** 03: Template config deployment; 04: Install OpenCode Desktop; 05: Optional development tools.

**Status:** ready-for-agent

- [ ] A clean-account test validates the full success path.
- [ ] Cancellation, invalid release signature/hash, template marker errors, and config overwrite confirmation leave expected files untouched.

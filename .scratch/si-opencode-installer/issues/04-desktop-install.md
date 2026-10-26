# 04: Install OpenCode Desktop

**What to build:** The installer reads the release tool manifest and installs OpenCode Desktop first. It skips an installed version that meets the release minimum and stops the installation if Desktop cannot be installed or verified.

**Blocked by:** 02: NAS release download.

**Status:** ready-for-agent

- [ ] Desktop installer hash, Authenticode signature, and installed version are verified.
- [ ] Desktop is mandatory and cannot be unchecked in the installation plan.

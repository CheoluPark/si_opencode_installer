# 02: NAS release download

**What to build:** The installer signs in to FileStation, reuses NAS credentials from Windows Credential Manager, downloads the signed current release to a temporary staging directory, and verifies it before changing the PC. Failed login prompts again; cancellation leaves the PC unchanged.

**Blocked by:** 01: Release structure and publishing.

**Status:** ready-for-agent

- [ ] A signed current release is downloaded and fully hash-verified before installation starts.
- [ ] NAS credentials are stored only in Windows Credential Manager and refreshed after successful login.

# 03: Template config deployment

**What to build:** The installer collects required LLM/wiki/work credentials and optional code PAT, replaces the approved template markers with those values and the target user profile path, validates the generated JSONC against the bundled OpenCode v2 schema, and deploys the complete config bundle after explicit overwrite confirmation.

**Blocked by:** 02: NAS release download.

**Status:** ready-for-agent

- [ ] Code PAT is visibly optional and its marked template block is removed when omitted.
- [ ] Existing OpenCode config is timestamp-backed-up before the full downloaded config bundle replaces it.

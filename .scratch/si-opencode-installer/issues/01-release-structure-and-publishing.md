# 01: Release structure and publishing

**What to build:** A release can be assembled in a staging folder, validated, signed, and promoted as the single current FileStation release. It contains the OpenCode config bundle, local schema, tool installers, hashes, sizes, and a signed release manifest. A release procedure documents the operator workflow.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] The release builder creates hashes, sizes, manifest, and Ed25519 signature from staged content.
- [ ] The documented process retains only `current` and temporary `staging` release folders.

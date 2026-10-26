# 05: Optional development tools

**What to build:** The installer presents manifest-driven installation choices for MSVC, Rust, Python, Node.js, and WSL after Desktop. It detects versions, defaults old/missing tools to selected, supports user skip and UAC cancellation, and requests at most one reboot at the end.

**Blocked by:** 02: NAS release download; 04: Install OpenCode Desktop.

**Status:** ready-for-agent

- [ ] User-scope tools install for the current user; machine-scope tools request elevation only when needed.
- [ ] Skipped, cancelled, or failed optional tools appear as warnings on the final screen.

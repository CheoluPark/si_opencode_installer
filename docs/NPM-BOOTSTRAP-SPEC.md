# SI OpenCode Nexus Bootstrap Specification

## Problem Statement

The current SI OpenCode Windows distribution is fully bundled so it can work in an air-gapped environment. This makes the release large because managed plugins, MCPs, LSP tools, skills, and their dependency trees are shipped alongside the mandatory large offline assets.

Corporate users can access approved Nexus npm and pip proxies, but many users cannot configure those proxies themselves. The distribution must exploit Nexus where it is available without making a Nexus failure, an unavailable package, or an unconfigured machine prevent installation. Large Windows and FPGA assets must remain directly delivered.

## Solution

Publish a small Node.js CommonJS command-line bootstrap package to the corporate Nexus npm registry. It prepares a local release payload and launches the existing signed SI OpenCode installer; it does not replace that installer.

The bootstrap package reads the signed release manifest and processes managed extension inventory entries. Entries manually designated as npm-installable are installed from the configured Nexus registry into a temporary directory, verified, and staged into the prepared payload. If npm is unavailable or installation fails, the bootstrap uses the matching file from a fixed `offline-payload` directory and verifies its SHA-256 hash. Entries not explicitly designated npm-installable use that direct-copy path immediately.

The existing signed installer remains responsible for Desktop installation, WSL2 recovery, Ubuntu provisioning, FPGA KB startup, user configuration preservation, updates, and release notes. The release bundle continues to directly carry the Desktop installer, WSL MSI, Ubuntu WSL image, and KB runtime sidecar.

## User Stories

1. As an SI OpenCode user, I want to run one bootstrap command so that I can prepare and start the normal installer without learning Nexus configuration.
2. As an SI OpenCode user, I want the existing installer to remain the final installer so that its Windows, WSL2, FPGA, updater, and preservation behavior does not change.
3. As a release operator, I want large installation assets delivered directly so that npm registry size and retention policies do not block releases.
4. As a release operator, I want reviewed managed extensions to be eligible for npm retrieval so that large dependency trees need not always be copied manually.
5. As a release operator, I want every npm-retrieved extension to have a direct-copy fallback so that a Nexus outage never blocks an otherwise complete offline release.
6. As an end user, I want a fixed offline-payload folder layout so that I can use a supplied release folder without choosing paths or setting environment variables.
7. As an end user, I want the bootstrap to use a directly supplied artifact whenever an extension is not marked npm-installable so that no unreviewed registry request occurs.
8. As an end user, I want npm failures to fall back automatically when a verified direct copy exists so that I do not need to diagnose registry failures.
9. As an end user, I want clear failure output when neither npm nor direct copy can supply a required item so that I can obtain the missing artifact.
10. As an end user, I want optional items to be reported but not block installation when they are unavailable.
11. As an administrator, I want the bootstrap to avoid permanent modification of my npm configuration so that it does not break existing development workflows.
12. As an administrator, I want temporary Nexus npm settings restored after execution so that credentials and registry overrides are not left behind unexpectedly.
13. As a security reviewer, I want direct-copy files checked against the signed manifest so that copied files cannot silently replace reviewed release content.
14. As a security reviewer, I want npm artifacts to be version-pinned and validated before staging so that a mutable latest tag cannot silently alter a release.
15. As a security reviewer, I want only manually approved npm packages to be retrieved so that registry package-name heuristics cannot introduce unreviewed software.
16. As a release operator, I want one build-time configuration value for the Nexus registry URL so that the placeholder can be replaced without source rewrites.
17. As a release operator, I want the published package name to be a build-time value so that the Nexus scope and naming convention can be finalized later.
18. As a release operator, I want packaging to restore placeholder source files after artifact creation so that source control remains registry-neutral.
19. As a release operator, I want the build to fail when a required npm-designated item has no direct-copy fallback so that releases preserve the offline guarantee.
20. As a test engineer, I want a fixture release tree with a fake npm executable so that npm success, npm failure, and direct-copy fallback can be verified without corporate Nexus access.
21. As an FPGA user, I want the KB runtime, WSL MSI, Ubuntu image, and Desktop installer to remain beside the installer so that the existing offline FPGA workflow continues to work.
22. As a non-FPGA user, I want the bootstrap not to make WSL or KB assets mandatory beyond the existing installer rules.
23. As an updater user, I want the bootstrap to leave signed release and updater behavior untouched so that update verification remains anchored in the current publisher key.
24. As a support engineer, I want an execution log identifying npm use, fallback use, hashes, and missing artifacts so that failed installations can be diagnosed from one record.

## Implementation Decisions

- Build a separate, dependency-free Node.js 18+ CommonJS CLI package with a single `si-opencode-bootstrap` command.
- Keep the bootstrap as an orchestrator only. It prepares payload inputs and invokes the existing installer; it must not duplicate installer, updater, WSL2, Ubuntu, Docker, or FPGA KB logic.
- Maintain a fixed release-root convention: direct artifacts live in an `offline-payload` directory whose relative paths mirror managed-payload paths.
- Continue to directly distribute the Desktop installer, WSL MSI, Ubuntu WSL image, KB runtime archive, installer EXE, and updater EXE. They are outside npm acquisition.
- Extend the signed managed-file manifest schema with optional delivery metadata: install method, npm package, pinned npm version, registry override, package-relative output path, direct-copy fallback flag, optionality, elevation indicator, and alternate direct path.
- Preserve backward compatibility: a manifest entry without delivery metadata is `directCopy`, required, and uses its existing manifest path.
- Begin with every managed entry designated `directCopy`. Move an entry to npm only through explicit manual approval after confirming availability through Nexus and defining its direct fallback.
- For an npm entry, create a temporary npm configuration scoped to the bootstrap invocation, run `npm install` into a temporary staging directory, and copy only the declared package-relative output artifact into the prepared payload. Restore any pre-existing npm configuration in all success and failure cases.
- Treat package retrieval, package version, expected staged paths, and expected hashes as release-controlled data. Do not infer package names or install method from file names.
- Verify direct-copy content with SHA-256 before it is staged. Verify staged output for both npm and direct-copy delivery before launching the installer.
- If npm retrieval fails and direct fallback is enabled, automatically attempt direct copy. If a required entry cannot be staged by either route, do not launch the installer. After the existing installer succeeds, copy the verified staged managed payload into its owned managed staging directory and invoke the signed updater's managed-install command; that command remains the only writer to user-level managed files.
- Emit a concise bootstrap log containing selected delivery method, npm command outcome, fallback decision, hash result, and installer launch result. Do not log credentials or tokens.
- Package the CLI independently from the Windows installer build. The release build performs placeholder replacement only during npm package creation, runs npm packaging, emits the tarball to release output, then restores source placeholders in a finally-equivalent cleanup path.
- The Nexus registry and package name are placeholders until corporate values are supplied. They must be centrally configurable at build time, not scattered through source.

## Testing Decisions

- Test at the highest seam: invoke the bootstrap CLI against a temporary fixture release root and inspect its externally observable staged payload, exit code, log, and whether the installer launcher was invoked.
- Use a fake npm executable or command shim rather than testing npm internals. It must support deterministic success, non-zero failure, and malformed-output scenarios.
- Test direct-copy staging with valid content, a missing file, a size mismatch, and a SHA-256 mismatch.
- Test npm success with a version-pinned fixture artifact; verify only declared output is staged.
- Test npm failure with a valid direct-copy fallback; confirm the bootstrap succeeds and records the fallback.
- Test npm failure without a direct-copy fallback for a required entry; confirm the bootstrap fails before invoking the installer.
- Test optional entry failure; confirm the bootstrap reports it and proceeds.
- Test temporary npm configuration creation and restoration for both successful and failing invocations, including an existing user configuration.
- Test manifest defaults so existing entries without delivery metadata retain direct-copy behavior.
- Test manifest rejection for invalid install method, npm entry without package/version, path traversal, or a required npm entry lacking fallback.
- Retain existing Rust manifest-signature, updater, and installer tests as regression coverage; the bootstrap must consume their signed manifest contract rather than weaken it.
- Complete a clean corporate-network Windows validation using the real Nexus endpoint after its URL and authentication scheme are supplied.

## Out of Scope

- Replacing Inno Setup or the Rust updater.
- Uploading multi-gigabyte Desktop, WSL, Ubuntu, or FPGA KB assets to Nexus npm.
- Adding pip installation logic in this first bootstrap. Pip availability remains a future delivery method; it must use the same explicit manifest and direct-copy fallback model if added.
- Auto-discovering or guessing npm-installable packages.
- Persisting Nexus credentials, replacing user-wide npm settings, or embedding access tokens in source, manifest, or logs.
- Changing NAS release download, Ed25519 release signing, updater authentication, release notes, or FPGA KB behavior.

## Further Notes

- The present release manifest is a signed inventory of managed files. Delivery metadata must be included in the signed material, or carried in an equally signed release-controlled companion document, before it is trusted by the bootstrap.
- The current build validates that every reviewed plugin, skill, MCP, and LSP asset is present in the managed payload. That validation should be relaxed only after bootstrap staging produces an equivalent verified prepared payload.
- The bootstrap's practical fallback guarantee requires release operators to bring `offline-payload` content for every package designated npm-installable.
- Issue tracker integration is not configured in this workspace. This specification is recorded locally until the project supplies its tracker and the `ready-for-agent` label mapping.

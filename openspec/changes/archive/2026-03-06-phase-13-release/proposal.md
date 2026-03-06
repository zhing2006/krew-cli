## Why

krew-cli has completed all 12 functional phases and is ready for its first public release (v0.1.0). Users currently must clone the repo and build from source, which limits adoption. We need production-quality static binaries for all major platforms and a frictionless install path via npm.

## What Changes

- Add `[profile.release]` optimizations: LTO, strip, codegen-units=1
- Configure static linking for all 5 target platforms (Windows x64, Linux x64/arm64, macOS x64/arm64)
- Add `mimalloc` as the allocator on Linux musl targets
- Create GitHub Actions workflow for automated cross-platform builds on tag push
- Create npm package structure with platform-specific optional dependencies (`@zhing2006/krew-*`) for binary distribution
- Add publish helper scripts for downloading release artifacts and publishing to npm

## Capabilities

### New Capabilities
- `static-linking`: Platform-specific static linking configuration (static_vcruntime on Windows, musl+mimalloc on Linux, crt-static on macOS) and release profile optimizations
- `ci-build`: GitHub Actions workflow that builds 5 platform targets on version tag push and creates a GitHub Release with binary artifacts
- `npm-distribution`: npm package structure with a main package (`@zhing2006/krew`) and 5 platform sub-packages using optionalDependencies pattern, plus JS shim and publish scripts

### Modified Capabilities

_None — this change adds build/distribution infrastructure without modifying existing runtime behavior._

## Impact

- **Cargo.toml**: Root workspace gains `[profile.release]` section; `krew-cli` crate gains conditional `mimalloc` dependency for Linux musl
- **New files**: `.github/workflows/release.yml`, `npm/` directory tree (6 package.json files + JS shim), `scripts/` helper scripts
- **Dependencies**: `mimalloc` (Linux only), `static_vcruntime` (already present for Windows)
- **No runtime behavior changes**: All modifications are build-time and distribution-level only

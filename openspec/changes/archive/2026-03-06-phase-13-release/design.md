## Context

krew-cli is a Rust workspace with 6 crates. The CLI binary crate (`krew-cli`) already uses `static_vcruntime` for Windows. All HTTP is via `reqwest` with `rustls` (no OpenSSL dependency). The project needs to produce fully static, single-file binaries for 5 platform targets and distribute them via GitHub Releases and npm.

## Goals / Non-Goals

**Goals:**
- Produce statically linked, optimized single-file binaries for all 5 targets
- Automate cross-platform builds via GitHub Actions on tag push
- Enable `npm install -g @zhing2026/krew` as the primary distribution channel
- Keep the publish step manual (human triggers npm publish)

**Non-Goals:**
- Homebrew tap, AUR, or other package managers (future work)
- Auto-publish to npm from CI (intentionally manual)
- Code signing or notarization (future work)
- Automated version bumping (manual for now)

## Decisions

### 1. Release profile optimizations

Use aggressive optimizations for smallest, fastest binary:

```toml
[profile.release]
lto = true
codegen-units = 1
strip = true
panic = "abort"
```

**Rationale**: `panic = "abort"` is safe here — the CLI is not a library, and unwinding is unnecessary. Combined with LTO and strip, this significantly reduces binary size.

### 2. Linux static linking via musl + mimalloc

Use `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl` targets. Replace the default musl allocator with `mimalloc` for better performance.

```toml
# In krew-cli/Cargo.toml
[target.'cfg(target_env = "musl")'.dependencies]
mimalloc = { version = "0.1", default-features = false }
```

**Rationale**: musl's default allocator has known performance issues with multi-threaded workloads. mimalloc is a drop-in replacement with minimal code changes (one `#[global_allocator]` annotation).

**Alternative considered**: jemalloc — heavier, more complex build, not needed for a CLI tool.

### 3. macOS static linking via RUSTFLAGS

Set `RUSTFLAGS="-C target-feature=+crt-static"` in the CI build step for macOS targets. No Cargo.toml changes needed.

**Rationale**: macOS static linking is best handled at build time via flags rather than in Cargo config, since it only applies to release builds in CI.

### 4. npm distribution via optionalDependencies pattern

Use the platform sub-package pattern (as used by esbuild, biome, oxlint):

```
@zhing2026/krew              → main package with JS shim + optionalDependencies
@zhing2026/krew-win32-x64    → Windows x64 binary
@zhing2026/krew-linux-x64    → Linux x64 binary
@zhing2026/krew-linux-arm64  → Linux arm64 binary
@zhing2026/krew-darwin-x64   → macOS x64 binary
@zhing2026/krew-darwin-arm64 → macOS arm64 binary
```

**Rationale**: Superior to postinstall-download approach — works with `--ignore-scripts`, offline installs, and corporate proxies. Industry standard for Rust/Go CLI tools distributed via npm.

**JS shim logic**: Maps `process.platform` + `process.arch` to sub-package name, resolves binary path, and exec's with inherited stdio.

### 5. GitHub Actions workflow on tag push

Trigger on `push: tags: ["v*"]`. Use a matrix strategy for 5 targets. Each job uploads its artifact. A final job creates a GitHub Release and attaches all binaries.

| Runner | Targets |
|--------|---------|
| `windows-latest` | `x86_64-pc-windows-msvc` |
| `ubuntu-latest` | `x86_64-unknown-linux-musl` |
| `ubuntu-latest` (cross) | `aarch64-unknown-linux-musl` |
| `macos-latest` (arm) | `aarch64-apple-darwin` |
| `macos-latest` (arm, cross) | `x86_64-apple-darwin` |

Linux arm64 cross-compilation uses `cross` or the `aarch64-linux-musl-cross` toolchain. macOS x64 can be cross-compiled from arm64 runner via `rustup target add`.

### 6. Publish helper scripts

Two bash scripts in `scripts/`:
- `prepare-npm.sh <version>`: Downloads release artifacts from GitHub via `gh release download`, renames and places binaries into the correct npm sub-package directories.
- `npm-publish.sh`: Publishes all 5 sub-packages first, then the main package, with `--access public`.

**Rationale**: Keeps npm publish manual and auditable. The scripts reduce tedium without removing human control.

## Risks / Trade-offs

- **[Cross-compilation failures]** → Linux arm64 and macOS x64 cross-compilation may have toolchain issues. Mitigation: Use well-tested `cross` tool for Linux arm64; macOS x64 cross-compile from arm64 is well-supported by Apple.
- **[macOS crt-static limitations]** → macOS doesn't fully support static linking of system libraries (libSystem is always dynamic). Mitigation: This is standard — all macOS CLI tools link libSystem dynamically. The binary still works without any additional installs.
- **[npm publish ordering]** → Sub-packages must be published before the main package. Mitigation: `npm-publish.sh` enforces correct order.
- **[Binary size]** → Full LTO increases compile time significantly. Mitigation: Only applies to release profile; development builds are unaffected.

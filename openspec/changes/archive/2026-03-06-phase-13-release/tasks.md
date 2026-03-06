## 1. Release Profile & Static Linking

- [x] 1.1 Add `[profile.release]` to root `Cargo.toml` with `lto = true`, `codegen-units = 1`, `strip = true`, `panic = "abort"`
- [x] 1.2 Add `mimalloc` dependency to `krew-cli/Cargo.toml` gated on `cfg(target_env = "musl")`
- [x] 1.3 Add `#[global_allocator]` mimalloc setup in `krew-cli/src/main.rs` gated on `cfg(target_env = "musl")`
- [x] 1.4 Verify local `cargo build --release` succeeds on Windows

## 2. GitHub Actions Workflow

- [x] 2.1 Create `.github/workflows/release.yml` with tag trigger (`on: push: tags: ["v*"]`)
- [x] 2.2 Define build matrix for 5 targets (win-x64, linux-x64, linux-arm64, darwin-x64, darwin-arm64) with runner, target triple, and binary name
- [x] 2.3 Add build steps: install Rust toolchain, install cross-compilation tools (musl-tools, cross for arm64), set RUSTFLAGS for macOS
- [x] 2.4 Add artifact upload steps for each target
- [x] 2.5 Add release job that downloads all artifacts and creates GitHub Release via `gh release create`

## 3. npm Package Structure

- [x] 3.1 Create `npm/krew/package.json` for main package `@zhing2026/krew` with `optionalDependencies` and `bin` entry
- [x] 3.2 Create `npm/krew/bin/krew` JS shim that resolves platform binary and exec's it
- [x] 3.3 Create `npm/krew-win32-x64/package.json` with `os: ["win32"]`, `cpu: ["x64"]`
- [x] 3.4 Create `npm/krew-linux-x64/package.json` with `os: ["linux"]`, `cpu: ["x64"]`
- [x] 3.5 Create `npm/krew-linux-arm64/package.json` with `os: ["linux"]`, `cpu: ["arm64"]`
- [x] 3.6 Create `npm/krew-darwin-x64/package.json` with `os: ["darwin"]`, `cpu: ["x64"]`
- [x] 3.7 Create `npm/krew-darwin-arm64/package.json` with `os: ["darwin"]`, `cpu: ["arm64"]`

## 4. Publish Scripts

- [x] 4.1 Create `scripts/prepare-npm.sh` to download release binaries via `gh` and place into npm sub-package directories
- [x] 4.2 Create `scripts/npm-publish.sh` to publish sub-packages then main package with `--access public`

## 5. Verification

- [x] 5.1 Verify `cargo build --release` produces working binary on Windows
- [x] 5.2 Verify npm package structure is valid (`npm pack --dry-run` on main and sub-packages)
- [x] 5.3 Update `docs/phases/phase-13-release.md` to mark as completed

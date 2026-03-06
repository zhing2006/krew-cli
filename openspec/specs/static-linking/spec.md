## ADDED Requirements

### Requirement: Release profile optimizations
The workspace root `Cargo.toml` SHALL define a `[profile.release]` section with `lto = true`, `codegen-units = 1`, `strip = true`, and `panic = "abort"`.

#### Scenario: Release build uses optimized profile
- **WHEN** `cargo build --release` is executed
- **THEN** the resulting binary uses LTO, single codegen unit, stripped symbols, and abort-on-panic

### Requirement: Windows static linking
The `krew-cli` crate SHALL depend on `static_vcruntime` for Windows MSVC targets, ensuring the binary does not require the Visual C++ Redistributable at runtime.

#### Scenario: Windows binary has no VC runtime dependency
- **WHEN** the release binary is built for `x86_64-pc-windows-msvc`
- **THEN** the binary runs on a clean Windows installation without the VC++ Redistributable installed

### Requirement: Linux musl static linking
The `krew-cli` crate SHALL be buildable with musl targets (`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`) producing a fully static binary with no dynamic library dependencies.

#### Scenario: Linux binary is fully static
- **WHEN** the release binary is built for `x86_64-unknown-linux-musl`
- **THEN** `ldd` reports "not a dynamic executable" (or equivalent)

### Requirement: Linux musl allocator override
The `krew-cli` crate SHALL use `mimalloc` as the global allocator when built for musl targets, replacing the default musl allocator.

#### Scenario: mimalloc is active on musl builds
- **WHEN** the binary is built for a `target_env = "musl"` target
- **THEN** the `#[global_allocator]` is set to `mimalloc::MiMalloc`

### Requirement: macOS static linking
macOS builds SHALL use `RUSTFLAGS="-C target-feature=+crt-static"` to statically link the C runtime. Both `x86_64-apple-darwin` and `aarch64-apple-darwin` targets MUST be supported.

#### Scenario: macOS binary runs without additional installs
- **WHEN** the release binary is built for `aarch64-apple-darwin` with crt-static
- **THEN** the binary runs on a stock macOS installation without additional library installs

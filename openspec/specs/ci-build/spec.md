## ADDED Requirements

### Requirement: Tag-triggered CI workflow
A GitHub Actions workflow SHALL trigger on pushes of tags matching `v*`. It MUST build release binaries for all 5 platform targets.

#### Scenario: Tag push triggers build
- **WHEN** a tag `v0.1.0` is pushed to the repository
- **THEN** GitHub Actions starts build jobs for `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`, `x86_64-apple-darwin`, and `aarch64-apple-darwin`

### Requirement: Matrix build strategy
The workflow SHALL use a matrix strategy to build all 5 targets in parallel, with each target specifying the appropriate runner, Rust target triple, and build flags.

#### Scenario: All targets build in parallel
- **WHEN** the release workflow runs
- **THEN** 5 build jobs execute concurrently, one per target platform

### Requirement: GitHub Release creation
The workflow SHALL create a GitHub Release for the tag and upload all 5 compiled binaries as release assets. Binary filenames MUST include the platform identifier (e.g., `krew-linux-x64`, `krew-darwin-arm64`, `krew-win32-x64.exe`).

#### Scenario: Release created with all binaries
- **WHEN** all 5 build jobs complete successfully
- **THEN** a GitHub Release is created for the tag with 5 binary assets attached

### Requirement: Linux arm64 cross-compilation
The workflow SHALL cross-compile for `aarch64-unknown-linux-musl` on an x86_64 Ubuntu runner using an appropriate cross-compilation toolchain.

#### Scenario: Linux arm64 binary is produced
- **WHEN** the `aarch64-unknown-linux-musl` build job runs on `ubuntu-latest`
- **THEN** a valid aarch64 Linux binary is produced

### Requirement: macOS x64 cross-compilation
The workflow SHALL cross-compile for `x86_64-apple-darwin` from an arm64 macOS runner using `rustup target add`.

#### Scenario: macOS x64 binary is produced
- **WHEN** the `x86_64-apple-darwin` build job runs on `macos-latest`
- **THEN** a valid x86_64 macOS binary is produced

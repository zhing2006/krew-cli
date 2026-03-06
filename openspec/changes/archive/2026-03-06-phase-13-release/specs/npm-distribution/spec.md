## ADDED Requirements

### Requirement: Main npm package
A main package `@zhing2026/krew` SHALL exist with `optionalDependencies` referencing all 5 platform sub-packages. It MUST declare a `bin` entry pointing to a JS shim script.

#### Scenario: npm install resolves platform binary
- **WHEN** a user runs `npm install -g @zhing2026/krew` on a supported platform
- **THEN** npm installs the main package and the matching platform sub-package

### Requirement: Platform sub-packages
Five platform sub-packages SHALL exist, each containing only the platform binary and a `package.json` with appropriate `os` and `cpu` fields:
- `@zhing2026/krew-win32-x64` (os: win32, cpu: x64)
- `@zhing2026/krew-linux-x64` (os: linux, cpu: x64)
- `@zhing2026/krew-linux-arm64` (os: linux, cpu: arm64)
- `@zhing2026/krew-darwin-x64` (os: darwin, cpu: x64)
- `@zhing2026/krew-darwin-arm64` (os: darwin, cpu: arm64)

#### Scenario: Only matching platform package is installed
- **WHEN** a user installs `@zhing2026/krew` on Linux x64
- **THEN** only `@zhing2026/krew-linux-x64` is installed as an optional dependency, not the other 4

### Requirement: JS shim executable
The main package SHALL include a JS shim at `bin/krew` that determines the current platform and architecture, resolves the binary path from the corresponding sub-package, and executes it with inherited stdio and exit code.

#### Scenario: JS shim launches correct binary
- **WHEN** a user runs `krew` after installing via npm on macOS arm64
- **THEN** the JS shim resolves `@zhing2026/krew-darwin-arm64/krew` and executes it

#### Scenario: JS shim reports unsupported platform
- **WHEN** a user runs `krew` on an unsupported platform/architecture combination
- **THEN** the shim prints an error message indicating the platform is not supported and exits with code 1

### Requirement: Publish helper scripts
A `scripts/prepare-npm.sh` script SHALL download release binaries from GitHub Releases (via `gh`) and place them into the correct npm sub-package directories. A `scripts/npm-publish.sh` script SHALL publish all sub-packages before the main package, all with `--access public`.

#### Scenario: Prepare script populates npm packages
- **WHEN** `scripts/prepare-npm.sh 0.1.0` is run after a GitHub Release exists for `v0.1.0`
- **THEN** each npm sub-package directory contains the correct platform binary

#### Scenario: Publish script publishes in correct order
- **WHEN** `scripts/npm-publish.sh` is run
- **THEN** all 5 sub-packages are published before the main `@zhing2026/krew` package

### Requirement: Version consistency
All 6 npm `package.json` files (1 main + 5 sub-packages) MUST use the same version number, and it MUST match the Cargo workspace version.

#### Scenario: Versions are aligned
- **WHEN** a release is prepared for version 0.1.0
- **THEN** all `package.json` files and `Cargo.toml` declare version `0.1.0`

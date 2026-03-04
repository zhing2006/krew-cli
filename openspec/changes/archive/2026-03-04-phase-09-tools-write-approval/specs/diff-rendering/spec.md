## ADDED Requirements

### Requirement: Diff line rendering with theme awareness
The diff rendering module SHALL render unified diff lines with background colors determined by the terminal's theme (dark or light). Dark theme SHALL use subtle background tints; light theme SHALL use GitHub-style pastel backgrounds.

#### Scenario: Dark theme colors
- **WHEN** terminal background is dark
- **THEN** insert lines SHALL use dark green background (#213A2B), delete lines SHALL use dark red background (#4A221D)

#### Scenario: Light theme colors
- **WHEN** terminal background is light
- **THEN** insert lines SHALL use pastel green background (#dafbe1), delete lines SHALL use pastel red (#ffebe9)

### Requirement: Color depth support
Diff rendering SHALL support three color depth levels:
- **TrueColor**: Full RGB colors
- **ANSI-256**: Quantized to nearest 256-color palette entry
- **ANSI-16**: Foreground-only styling (no background tints)

The color level SHALL be detected automatically from terminal capabilities.

#### Scenario: TrueColor terminal
- **WHEN** terminal supports TrueColor
- **THEN** diff SHALL use RGB background colors

#### Scenario: ANSI-256 terminal
- **WHEN** terminal supports only 256 colors
- **THEN** diff SHALL use nearest ANSI-256 palette entries for backgrounds

#### Scenario: ANSI-16 terminal
- **WHEN** terminal supports only 16 colors
- **THEN** diff SHALL use foreground-only styling (green for +, red for -)

### Requirement: Diff line layout
Each diff line SHALL consist of three visual regions: gutter (right-aligned line number), sign character (+/-/space), and content text.

#### Scenario: Insert line
- **WHEN** rendering an insert line at line 42
- **THEN** SHALL display `42  + <content>` with green styling

#### Scenario: Delete line
- **WHEN** rendering a delete line at line 10
- **THEN** SHALL display `10  - <content>` with red styling

#### Scenario: Context line
- **WHEN** rendering a context line
- **THEN** SHALL display with default styling and space sign

### Requirement: Syntax highlighting in diffs
Diff rendering SHALL support optional syntax highlighting of code within diff lines using `syntect`. Insert lines SHALL use normal syntax colors; delete lines SHALL use dimmed syntax colors.

#### Scenario: Highlighted Rust code
- **WHEN** rendering a diff of a `.rs` file with syntax highlighting enabled
- **THEN** keywords, strings, and other tokens SHALL be colored according to the syntax theme, overlaid on the diff background

#### Scenario: Highlighting limits
- **WHEN** diff content exceeds 512KB or 10,000 lines
- **THEN** syntax highlighting SHALL be skipped and plain text rendering used

### Requirement: Unicode-aware line wrapping
Diff lines exceeding the terminal width SHALL be wrapped at character boundaries, preserving styles across wrapped segments. Tab characters SHALL be counted as 4 display columns.

#### Scenario: Long line wrapping
- **WHEN** a diff line exceeds terminal width
- **THEN** it SHALL be wrapped to multiple display lines with continuation indent

#### Scenario: CJK character width
- **WHEN** a diff line contains CJK characters (width 2)
- **THEN** wrapping SHALL account for double-width characters

### Requirement: Diff summary for multiple files
When displaying diffs for multiple file changes, the renderer SHALL produce a summary header showing the number of files changed and per-file addition/deletion counts.

#### Scenario: Multi-file summary
- **WHEN** displaying diffs for 3 files with total +10/-5 changes
- **THEN** summary SHALL show file count and per-file (+N/-M) statistics

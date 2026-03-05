## ADDED Requirements

### Requirement: fetch_url tool definition
The system SHALL provide a built-in tool named `fetch_url` that accepts a single required parameter `url` (string) and returns the web page content converted to Markdown.

#### Scenario: Fetch a valid HTTPS URL
- **WHEN** the agent calls `fetch_url` with `url: "https://example.com"`
- **THEN** the tool SHALL fetch the page, convert HTML to Markdown using htmd, and return the Markdown content

#### Scenario: Fetch with HTTP URL auto-upgrade
- **WHEN** the agent calls `fetch_url` with `url: "http://example.com"`
- **THEN** the tool SHALL upgrade the URL to HTTPS before fetching

#### Scenario: Invalid URL
- **WHEN** the agent calls `fetch_url` with an invalid URL
- **THEN** the tool SHALL return a ToolResult with `is_error = true` and a descriptive error message

### Requirement: fetch_url response size limit
The tool SHALL limit the response body to 1MB. Content exceeding this limit SHALL be truncated with a notice appended.

#### Scenario: Response exceeds 1MB
- **WHEN** the fetched page response body exceeds 1MB
- **THEN** the tool SHALL truncate the content at 1MB and append a notice indicating truncation

#### Scenario: Response within limit
- **WHEN** the fetched page response body is within 1MB
- **THEN** the tool SHALL return the full converted Markdown content

### Requirement: fetch_url approval mechanism
The `fetch_url` tool SHALL require user approval by default. Domains listed in `fetch_allow_domains` in `settings.toml` SHALL be auto-approved without user confirmation.

#### Scenario: Domain not in whitelist
- **WHEN** the agent calls `fetch_url` with a URL whose domain is NOT in `fetch_allow_domains`
- **THEN** the tool SHALL require user approval before execution

#### Scenario: Domain in whitelist
- **WHEN** the agent calls `fetch_url` with a URL whose domain matches an entry in `fetch_allow_domains`
- **THEN** the tool SHALL execute without requiring user approval

#### Scenario: Subdomain matching
- **WHEN** `fetch_allow_domains` contains `github.com` and the URL is `https://docs.github.com/some/path`
- **THEN** the tool SHALL auto-approve because `docs.github.com` ends with `github.com`

### Requirement: fetch_url network behavior
The tool SHALL follow redirects, use a 30-second timeout, and set User-Agent to `krew-cli/0.1.0`.

#### Scenario: Redirect followed
- **WHEN** the fetched URL returns a 3xx redirect
- **THEN** the tool SHALL follow the redirect and return the final page content

#### Scenario: Request timeout
- **WHEN** the server does not respond within 30 seconds
- **THEN** the tool SHALL return a ToolResult with `is_error = true` indicating timeout

### Requirement: fetch_allow_domains configuration
The `settings.toml` SHALL support a `fetch_allow_domains` field (array of strings) at the top level, alongside `shell_allow_commands`.

#### Scenario: Config with whitelist
- **WHEN** `settings.toml` contains `fetch_allow_domains = ["docs.rs", "github.com"]`
- **THEN** the config system SHALL parse and provide these domains to the fetch_url tool

#### Scenario: Config without whitelist
- **WHEN** `settings.toml` does not contain `fetch_allow_domains`
- **THEN** the field SHALL default to an empty array (all domains require approval)

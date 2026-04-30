### Requirement: krew config help 子命令
`krew-cli` SHALL 提供 `krew config help` 子命令，在标准输出打印完整的 krew 配置手册（英文）。手册内容 SHALL 为硬编码的纯文本，与代码中的配置模型完全一致。该命令 SHALL 不读取任何配置文件，不需要网络访问，直接打印静态文本后以退出码 0 退出。

#### Scenario: 执行 krew config help
- **WHEN** 执行 `krew config help`
- **THEN** SHALL 在标准输出打印完整的配置手册文本（英文），退出码为 0

#### Scenario: 手册包含文件位置和 merge 规则说明
- **WHEN** 执行 `krew config help`
- **THEN** 输出 SHALL 包含 user config 路径（`~/.krew/settings.toml`）、project config 路径（`.krew/settings.toml`）以及两层配置的 merge 规则说明

#### Scenario: 手册准确描述各层支持的 section
- **WHEN** 执行 `krew config help`
- **THEN** 输出 SHALL 说明 user config 支持 `[settings]`（不含 reply_order）、`[providers.*]`、`[[mcp_servers]]`、`[skills]`；project config 额外支持 `[[agents]]` 和 `reply_order`

#### Scenario: 手册包含 settings 完整字段说明
- **WHEN** 执行 `krew config help`
- **THEN** 输出 SHALL 包含 `[settings]` 表的所有字段：approval_mode、reply_order（仅 project）、auto_compact_threshold、compact_keep_rounds、input_history_limit、paste_burst_detection、worker_threads、other_agent_role、shell_allow_commands、fetch_allow_domains、agent_to_agent_routing、agent_to_agent_max_rounds、language、restrict_workspace、**update_check**，以及各字段的默认值（与代码常量一致）

#### Scenario: 手册包含 providers 完整字段说明
- **WHEN** 执行 `krew config help`
- **THEN** 输出 SHALL 包含 `[providers.<name>]` 表的所有字段：type、api_key、api_key_env、base_url、vertex_project、vertex_location

#### Scenario: 手册包含 agents 完整字段说明
- **WHEN** 执行 `krew config help`
- **THEN** 输出 SHALL 包含 `[[agents]]` 数组表的所有字段：name、display_name、provider、model、api_type、color、system_prompt、tools、enable_web_search、sampling、enable_thinking、thinking_effort，以及各字段的默认值（与代码常量一致）

#### Scenario: 手册包含 mcp_servers 完整字段说明
- **WHEN** 执行 `krew config help`
- **THEN** 输出 SHALL 包含 `[[mcp_servers]]` 数组表的所有字段：name、command、args、env、url、headers、trust

#### Scenario: 手册包含 skills 完整字段说明
- **WHEN** 执行 `krew config help`
- **THEN** 输出 SHALL 包含 `[skills]` 表的所有字段：enabled、extra_paths，以及默认值

#### Scenario: 手册包含 retry 完整字段说明
- **WHEN** 执行 `krew config help`
- **THEN** 输出 SHALL 包含 `[settings.retry]` 表的所有字段：max_retries_rate_limit、max_retries_server_error、backoff_base_secs、backoff_multiplier、server_error_interval_secs、request_timeout_secs，以及默认值

#### Scenario: 手册包含 sampling 完整字段说明
- **WHEN** 执行 `krew config help`
- **THEN** 输出 SHALL 包含 sampling 子表的所有字段：temperature、top_p、top_k、max_tokens、frequency_penalty、presence_penalty、stop_sequences

#### Scenario: 手册包含 CLI 命令参考
- **WHEN** 执行 `krew config help`
- **THEN** 输出 SHALL 包含 `krew config init`、`add`、`del`、`list`、`doctor`、`help` 命令的简要说明

#### Scenario: 手册包含示例配置
- **WHEN** 执行 `krew config help`
- **THEN** 输出 SHALL 包含至少一个 user config 示例片段和至少一个 project config 示例片段


### Requirement: config help documents Vertex Anthropic
`krew config help` SHALL document `vertex-anthropic` as a supported provider type and explain its field semantics.

#### Scenario: Provider type list includes vertex-anthropic
- **WHEN** executing `krew config help`
- **THEN** the providers section SHALL list `vertex-anthropic` alongside `openai`、`anthropic` and `google`

#### Scenario: Vertex Anthropic field semantics
- **WHEN** executing `krew config help`
- **THEN** the help text SHALL explain that `api_key` / `api_key_env` are Bearer token sources for `vertex-anthropic`
- **AND** SHALL explain that `base_url` is optional and can point to a LiteLLM Vertex passthrough root
- **AND** SHALL explain that `vertex_project` and `vertex_location` are required for runtime use

#### Scenario: Vertex Anthropic example
- **WHEN** executing `krew config help`
- **THEN** the example configurations SHALL include a `[providers.vertex-anthropic]` block using `type = "vertex-anthropic"`

## ADDED Requirements

### Requirement: CLI 交互语言为英文
所有 `krew config` 子命令的交互式提示文本、选项标签、成功/失败消息 SHALL 使用英文。这包括 dialoguer 的 prompt 文本、Select 选项、Confirm 提示、错误提示和摘要输出。

#### Scenario: Select 提示为英文
- **WHEN** 显示供应商类型选择
- **THEN** prompt 文本 SHALL 为英文，如 `"Select provider type:"`

#### Scenario: Confirm 提示为英文
- **WHEN** 提示用户是否继续添加
- **THEN** prompt 文本 SHALL 为英文，如 `"Add another provider?"`

#### Scenario: 成功消息为英文
- **WHEN** 供应商添加成功
- **THEN** 输出 SHALL 为英文，如 `"Added provider \"anthropic\" (Anthropic)"`

#### Scenario: 错误提示为英文
- **WHEN** 供应商名称已存在
- **THEN** 提示 SHALL 为英文，如 `"Provider name \"anthropic\" already exists, please enter a different name"`

#### Scenario: 摘要表格标题为英文
- **WHEN** 显示供应商或 Agent 摘要表格
- **THEN** 列标题 SHALL 为英文，如 `"Name"`, `"Type"`, `"Key Method"`

### Requirement: init 智能分流
`krew config init` SHALL 根据配置文件存在状态自动分流（bootstrap 语义：只初始化不存在的配置）：
- user 不存在，project 不存在 → User Init，完成后提示衔接 Project Init
- user 不存在，project 存在 → 仅 User Init（project 已有，不再初始化）
- user 存在，project 不存在 → 仅 Project Init
- user 存在，project 存在 → 提示配置已存在，退出

#### Scenario: 两个配置文件均不存在
- **WHEN** `~/.krew/settings.toml` 和 `.krew/settings.toml` 均不存在
- **THEN** SHALL 进入 User Init 流程（供应商配置）

#### Scenario: user 不存在但 project 存在
- **WHEN** `~/.krew/settings.toml` 不存在且 `.krew/settings.toml` 已存在
- **THEN** SHALL 仅进入 User Init 流程（供应商配置），完成后 SHALL 不提示衔接 Project Init（因为 project 配置已存在）

#### Scenario: 仅 user 配置存在
- **WHEN** `~/.krew/settings.toml` 存在且 `.krew/settings.toml` 不存在
- **THEN** SHALL 跳过 User Init，直接进入 Project Init 流程（Agent 配置）

#### Scenario: 两个配置文件均存在
- **WHEN** `~/.krew/settings.toml` 和 `.krew/settings.toml` 均存在
- **THEN** SHALL 输出 "Configuration already exists. Use `krew config add/del` to modify."，然后退出

#### Scenario: User Init 完成后衔接 Project Init（仅当 project 不存在时）
- **WHEN** User Init 流程完成（至少添加了一个供应商）且 `.krew/settings.toml` 不存在
- **THEN** SHALL 提示 "Initialize agent configuration for current project?"，用户确认后进入 Project Init 流程

#### Scenario: User Init 完成后 project 已存在时不提示衔接
- **WHEN** User Init 流程完成且 `.krew/settings.toml` 已存在
- **THEN** SHALL 打印成功信息并正常退出，不提示 Project Init

#### Scenario: User Init 完成后用户拒绝 Project Init
- **WHEN** User Init 完成后提示衔接 Project Init，用户选择不继续
- **THEN** SHALL 打印成功信息并正常退出

### Requirement: User Init 供应商配置循环
User Init 流程 SHALL 以循环方式让用户添加一个或多个供应商。每次循环包含以下步骤：
1. `Select` 选择供应商类型（Anthropic / OpenAI / Google / OpenAI-Compatible）
2. `Input` 输入供应商名称（带根据类型自动生成的默认值）
3. `Select` 选择 API Key 存储方式（环境变量 / 写入配置文件）
4. 根据存储方式收集 key（`Input` 输入环境变量名或 `Password` 输入 API key）
5. 如果是 OpenAI 兼容类型，额外 `Input` 输入 base_url
6. 如果是 Google 类型，`Select` 选择接入方式（Gemini API / Vertex AI），Vertex AI 额外收集 project_id 和 location
7. `Confirm` 是否继续添加下一个供应商

#### Scenario: 添加 Anthropic 供应商（环境变量方式）
- **WHEN** 用户选择 Anthropic 类型，选择环境变量存储
- **THEN** SHALL 自动建议供应商名称为 `anthropic`，环境变量名为 `ANTHROPIC_API_KEY`
- **AND** SHALL 将供应商写入 `~/.krew/settings.toml`，配置包含 `type = "anthropic"` 和 `api_key_env = "ANTHROPIC_API_KEY"`

#### Scenario: 添加 OpenAI 供应商（写入配置方式）
- **WHEN** 用户选择 OpenAI 类型，选择写入配置文件
- **THEN** SHALL 使用 `Password` 组件遮罩输入
- **AND** SHALL 将 `api_key = "sk-..."` 写入 `~/.krew/settings.toml`

#### Scenario: 添加 OpenAI 兼容供应商
- **WHEN** 用户选择 "OpenAI-Compatible" 类型
- **THEN** SHALL 额外提示输入 `base_url`
- **AND** SHALL 写入 `type = "openai"` 和 `base_url = "..."` 到配置

#### Scenario: 添加 Google Vertex AI 供应商
- **WHEN** 用户选择 Google 类型并选择 Vertex AI 接入方式
- **THEN** SHALL 额外收集 `vertex_project` 和 `vertex_location`
- **AND** SHALL 写入包含 `type = "google"`、`vertex_project`、`vertex_location` 的配置

#### Scenario: 供应商名称重复
- **WHEN** 用户输入的供应商名称已存在于 `~/.krew/settings.toml`
- **THEN** SHALL 提示名称已存在，要求用户重新输入

#### Scenario: 循环添加多个供应商
- **WHEN** 用户完成第一个供应商后选择继续
- **THEN** SHALL 重新进入添加流程，计数器显示 "Add provider [2]"
- **AND** 完成后再次提示是否继续

#### Scenario: 循环结束后显示摘要
- **WHEN** 用户选择不再继续添加
- **THEN** SHALL 打印已添加供应商的摘要表格（名称、类型、Key 方式）

#### Scenario: 供应商名称自动生成
- **WHEN** 用户选择供应商类型为 Anthropic
- **THEN** SHALL 自动建议名称 `anthropic`
- **WHEN** 已存在名为 `anthropic` 的供应商且用户再次选择 Anthropic 类型
- **THEN** SHALL 自动建议名称 `anthropic-2`

### Requirement: Project Init Agent 配置
Project Init 流程 SHALL 提供两种 Agent 创建方式：智能预设和手动创建。

#### Scenario: 选择创建方式
- **WHEN** 进入 Project Init 流程
- **THEN** SHALL 先列出当前可用供应商，然后显示 `Select` 让用户选择 "Smart Preset" 或 "Manual Setup"

#### Scenario: 无可用供应商
- **WHEN** 进入 Project Init 但 merge 后的配置（user + project）中没有任何供应商
- **THEN** SHALL 提示 "No providers configured. Run `krew config add provider` first." 并退出

#### Scenario: 供应商来源包含 user 和 project
- **WHEN** user 配置有供应商 `anthropic`，project 配置有供应商 `openai`
- **THEN** 可用供应商列表 SHALL 包含 `anthropic` 和 `openai`（合并两层配置）

#### Scenario: project 级供应商覆盖 user 级
- **WHEN** user 和 project 配置都定义了同名供应商 `openai`
- **THEN** SHALL 使用 project 级的供应商配置（与运行时 merge 行为一致）

### Requirement: 智能预设
智能预设 SHALL 在创建前获取所有已配置供应商的可用模型列表。预设方案仅两种：「单 Agent」和「三 Agent」。

#### Scenario: 获取模型列表
- **WHEN** 进入智能预设流程
- **THEN** SHALL 对每个已配置供应商调用 List Models API（带 spinner 提示 "Fetching available models..."），收集所有 (供应商, 模型) 候选对

#### Scenario: 所有供应商的模型获取均失败
- **WHEN** 所有供应商的 List Models API 调用均失败或返回空列表，且 fallback 列表也为空
- **THEN** SHALL 提示 "Failed to fetch available models. Check provider configuration or use manual creation."

#### Scenario: 候选模型数 >= 3 显示两种预设
- **WHEN** 收集到 >= 3 个不同的候选模型
- **THEN** SHALL 显示 `Select` 包含 "Single Agent" 和 "Three Agents" 两个选项

#### Scenario: 候选模型数 1-2 仅显示单 Agent 预设
- **WHEN** 收集到 1-2 个候选模型
- **THEN** SHALL 仅显示 "Single Agent" 选项（不显示 "Three Agents"）

#### Scenario: 单 Agent 预设选择
- **WHEN** 用户选择 "Single Agent" 预设
- **THEN** SHALL 显示 `Select` 列出所有候选模型（格式 `model_name (provider_name)`），用户选择一个

#### Scenario: 三 Agent 预设选择
- **WHEN** 用户选择 "Three Agents" 预设
- **THEN** SHALL 依次显示 3 次 `Select`（每次从剩余候选中选择），每次选择后从候选列表中移除已选模型

#### Scenario: 预设生成的 Agent 属性自动推导
- **WHEN** 通过预设选择了模型
- **THEN** Agent 名称 SHALL 从模型名提取前缀（如 `claude-opus-4-6` → `claude`），display_name 为首字母大写，颜色从 `[blue, green, cyan, magenta, yellow, red, white]` 按序分配，enable_thinking 默认 true，enable_web_search 默认 false，tools 默认 true

#### Scenario: Agent 名称去重
- **WHEN** 推导出的 Agent 名称与已有名称冲突
- **THEN** SHALL 自动追加数字后缀（如 `claude-2`）

#### Scenario: 预设完成后显示摘要并确认
- **WHEN** 预设流程完成
- **THEN** SHALL 打印生成的 Agent 列表表格和 reply_order，然后提示 `Confirm` 确认写入

### Requirement: 手动逐个创建 Agent
手动创建 SHALL 以循环方式让用户逐个添加 Agent。

#### Scenario: 手动创建一个 Agent
- **WHEN** 用户选择手动创建
- **THEN** SHALL 依次执行：`Select` 供应商 → `Select/FuzzySelect` 模型 → `Input` Agent 名称（带自动默认值）→ `Input` 显示名称（带自动默认值）→ `Select` 颜色 → `Confirm` 启用 thinking → `Confirm` 启用 web search

#### Scenario: 供应商的模型获取失败时降级
- **WHEN** 用户选择了某供应商但获取模型列表失败
- **THEN** SHALL 使用 fallback 列表展示选择，如果 fallback 也无对应项，则用 `Input` 让用户手动输入模型名

#### Scenario: 手动创建循环
- **WHEN** 完成一个 Agent 的创建
- **THEN** SHALL 提示 "Add another agent?"，确认后重新进入创建流程

#### Scenario: 手动创建结束后写入
- **WHEN** 用户选择不再继续添加
- **THEN** SHALL 将所有 Agent 写入 `.krew/settings.toml`，reply_order 按创建顺序排列

#### Scenario: 颜色选择排除已用颜色
- **WHEN** 已有 Agent 使用了 `blue` 颜色
- **THEN** 颜色选择列表 SHALL 将 `blue` 排在末尾或标记为已用，但仍允许选择

### Requirement: init --user / --project 强制标志
`krew config init` SHALL 支持 `--user` 和 `--project` 可选标志，跳过智能分流直接进入对应流程。`init` 的语义是 bootstrap（初始化），不是编辑。

#### Scenario: --user 强制进入 User Init
- **WHEN** 执行 `krew config init --user` 且 `~/.krew/settings.toml` 不存在
- **THEN** SHALL 直接进入 User Init 供应商配置流程

#### Scenario: --user 且 user 配置已存在
- **WHEN** 执行 `krew config init --user` 且 `~/.krew/settings.toml` 已存在
- **THEN** SHALL 提示 "User config already exists. Use `krew config add/del provider` to modify." 并退出

#### Scenario: --project 强制进入 Project Init
- **WHEN** 执行 `krew config init --project` 且 `.krew/settings.toml` 不存在
- **THEN** SHALL 直接进入 Project Init Agent 配置流程

#### Scenario: --project 且 project 配置已存在
- **WHEN** 执行 `krew config init --project` 且 `.krew/settings.toml` 已存在
- **THEN** SHALL 提示 "Project config already exists. Use `krew config add/del agent` to modify." 并退出

#### Scenario: --user 和 --project 互斥
- **WHEN** 执行 `krew config init --user --project`
- **THEN** SHALL 报错提示两个标志互斥


### Requirement: User Init supports Vertex Anthropic provider
`krew config init` User Init provider loop SHALL include `Vertex Anthropic` as a provider type option and collect the fields required to call Claude on Vertex AI.

#### Scenario: Select Vertex Anthropic provider type
- **WHEN** User Init shows the provider type selection
- **THEN** the options SHALL include `Vertex Anthropic`

#### Scenario: Default provider name and key env
- **WHEN** the user selects `Vertex Anthropic`
- **THEN** the default provider name SHALL be `vertex-anthropic`
- **AND** the default environment variable name SHALL be `VERTEX_ANTHROPIC_API_KEY`

#### Scenario: Collect Vertex fields
- **WHEN** the user selects `Vertex Anthropic`
- **THEN** User Init SHALL prompt for `Vertex AI project ID`
- **AND** SHALL prompt for `Vertex AI location` with default `global`

#### Scenario: Optional passthrough base_url
- **WHEN** the user selects `Vertex Anthropic`
- **THEN** User Init SHALL allow an empty `Base URL`
- **AND** an empty value SHALL mean Google official Vertex endpoint
- **AND** a non-empty value SHALL be written as `base_url`

#### Scenario: Write Vertex Anthropic provider config
- **WHEN** the user completes Vertex Anthropic provider setup
- **THEN** User Init SHALL write a provider with `type = "vertex-anthropic"`、`api_key_env` or `api_key`、`vertex_project` and `vertex_location`

### Requirement: Smart Preset includes Vertex Anthropic models
Project Init Smart Preset SHALL treat `vertex-anthropic` providers as model sources through `list_models` and SHALL allow selected Claude models to become agents.

#### Scenario: Fetch Vertex Anthropic models
- **WHEN** Smart Preset fetches available models for a configured `vertex-anthropic` provider
- **THEN** it SHALL call `list_models` with `ProviderType::VertexAnthropic`

#### Scenario: Create agent from Vertex Anthropic model
- **WHEN** the user selects model `claude-opus-4-7` from provider `vertex-anthropic`
- **THEN** the generated agent SHALL reference provider `vertex-anthropic` and model `claude-opus-4-7`

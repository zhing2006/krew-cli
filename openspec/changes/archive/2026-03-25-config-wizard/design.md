## Context

krew-cli 目前仅支持手动编辑 TOML 配置文件。配置分为两层：user 级 `~/.krew/settings.toml`（供应商定义）和 project 级 `.krew/settings.toml`（Agent 定义 + settings）。现有的 `krew-config` crate 仅有读取/反序列化能力，无写入能力。`krew-llm` 仅有 `chat_stream()` 接口，无 `list_models` 能力。

CLI 入口目前使用 clap 的扁平参数结构（`--config`、`--agents`、`--resume` 等），无子命令。

## Goals / Non-Goals

**Goals:**
- 通过交互式向导实现零手写 TOML 的配置体验
- 分离 user 级（供应商）和 project 级（Agent）配置的管理流程
- 提供 CRUD + 诊断的完整配置管理命令集
- 配置文件写入时保留用户已有的注释和格式
- 通过 List Models API 让用户从实际可用模型中选择

**Non-Goals:**
- 不修改 TUI 主流程、Agent Loop、消息路由等核心运行时
- 不支持配置文件的 GUI 编辑器
- 不在运行时热重载配置（仍需重启）
- 不做供应商 API 的连通性测试（仅 doctor 检查 key 是否存在）

## Decisions

### D1: 子命令结构使用 `krew config <action>` 分组式

使用 clap 的嵌套 subcommand 结构。`krew`（无子命令）保持直接进入 TUI 的现有行为。

```
krew                          → TUI（现有行为不变）
krew config init              → 交互式初始化
krew config add provider      → 添加供应商
krew config add agent         → 添加 Agent
krew config del provider      → 删除供应商
krew config del agent         → 删除 Agent
krew config list providers    → 列出供应商
krew config list agents       → 列出 Agent
krew config doctor            → 诊断配置
```

**替代方案**：扁平式 `krew init` / `krew add provider`。放弃原因：随着命令增多会与主流程参数混杂，分组更清晰。

### D2: 交互库选择 dialoguer

使用 `dialoguer` crate 的 `Select`、`Confirm`、`Input`、`Password` 组件。需要 `fuzzy-select` feature 用于模型选择（模型列表可能较长）。

**替代方案**：`inquire` crate。放弃原因：`dialoguer` 生态更成熟，下载量更大，自定义 theme 能力更好。

### D3: TOML 写入使用 toml_edit

使用 `toml_edit` crate 进行格式保留编辑。在 `krew-config` 中新增写入模块，提供面向业务的写入 API（add_provider、remove_provider、add_agent、remove_agent 等），内部使用 `toml_edit::DocumentMut` 操作 TOML AST。

文件不存在时创建新文件；文件存在时解析为 `DocumentMut` 后修改再写回，保留注释和格式。

**替代方案**：`toml::ser` 序列化覆盖。放弃原因：会丢失用户手动添加的注释和格式。

### D4: List Models 放在 krew-llm 中

在 `krew-llm` 中新增 `list_models(config: &ListModelsConfig) -> Result<Vec<ModelInfo>>` 异步函数。`ListModelsConfig` 包含 provider_type、base_url、api_key、vertex_project、vertex_location。各供应商端点：

| 供应商 | 端点 | 认证 |
|--------|------|------|
| OpenAI | `GET /v1/models` | `Authorization: Bearer <key>` |
| Anthropic | `GET /v1/models` | `X-Api-Key: <key>` + `anthropic-version` header |
| Google | `GET /v1beta/models` | `?key=<key>` query param |

过滤规则：OpenAI 仅保留 `gpt-*` / `o*` / `chatgpt-*` 开头的模型；Anthropic 仅保留 `claude-*`；Google 仅保留 `gemini-*`。

Vertex AI 路径：当检测到 `vertex_project` 和 `vertex_location` 已设置时，调用 Vertex AI 的 List Publisher Models 端点（`GET https://{location}-aiplatform.googleapis.com/v1/projects/{project}/locations/{location}/publishers/google/models`），使用 Bearer token 认证，与现有运行时 `GoogleClient` 的 Vertex 模式一致。

函数签名使用 `ListModelsConfig` 结构体而非散装参数，以承载 Vertex AI 所需的额外字段。

Fallback 硬编码列表（API 调用失败或超时 5 秒时使用）：

```
Anthropic:  claude-opus-4-6, claude-sonnet-4-6, claude-haiku-4-5-20251001
OpenAI:     gpt-5.4, gpt-5.4-mini, gpt-5.4-nano
Google:     gemini-3.1-pro-preview, gemini-3.1-flash-lite-preview
```

**替代方案**：在 `krew-cli` 中直接用 reqwest 裸调。放弃原因：会与 `krew-llm` 中已有的 auth header 逻辑重复。

### D5: init 的智能分流逻辑（bootstrap 语义）

`init` 的语义是 bootstrap（初始化），不是编辑器。已有配置时拒绝操作，引导到 `add/del`。

```
krew config init
  ├─ user ✗  project ✗
  │   → User Init → 完成后提示衔接 Project Init
  │
  ├─ user ✗  project ✓
  │   → 仅 User Init（project 已有，不再初始化）
  │
  ├─ user ✓  project ✗
  │   → 仅 Project Init
  │
  └─ user ✓  project ✓
      → 提示配置已存在，引导使用 add/del/list
```

`--user` / `--project` 标志也遵循同样的 bootstrap 语义：如果对应配置文件已存在则拒绝。

### D6: 智能预设仅两种

- **单 Agent**：用户从所有可用 (provider, model) 对中选一个
- **三 Agent**：用户依次选 3 个不同的 (provider, model) 对

预设可用性由实际获取到的模型数量决定：
- 候选模型 >= 1：可选「单 Agent」
- 候选模型 >= 3：可同时选「三 Agent」
- 候选模型 = 0：无法创建预设，提示用户检查供应商配置

### D7: provider/agent 隐含目标文件，不需要 --user/--project flag

`add/del/list provider` 操作 `~/.krew/settings.toml`；`add/del/list agent` 操作 `.krew/settings.toml`。命令本身已包含语义，无需额外 flag。

注意：`add agent` 和 `init` 的 Project Init 在列出可用供应商时，SHALL 读取 merge 后的配置（user + project），与运行时行为一致。这意味着 project 级定义的 provider 也会出现在可选列表中。`doctor` 同样基于 merge 后的最终配置进行诊断。

### D8: Agent 属性自动推导

| 属性 | 推导规则 |
|------|---------|
| name | 从模型名提取前缀：`claude-opus-4-6` → `claude`，重名时加后缀 `-2` |
| display_name | name 首字母大写 |
| color | 从预定义色板 `[blue, green, cyan, magenta, yellow, red, white]` 按序分配 |
| enable_thinking | 默认 `true`（见 D9） |
| enable_web_search | 默认 `false` |
| tools | 默认 `true` |

用户可通过后续 `Confirm` 步骤修改 thinking 和 web_search。

### D9: Wizard 中 enable_thinking 默认为 true（显式偏离 config 默认值）

`AgentConfig` 的 `enable_thinking` 在配置模型层默认为 `false`（`krew-config` 的 `#[serde(default)]`），这是为了向后兼容——不在 TOML 中显式写 `enable_thinking = true` 的老配置不会意外开启 thinking。

但 wizard 是面向**新用户的引导流程**，新用户大概率希望使用模型的 thinking 能力。因此 wizard 的预设和手动创建中 `enable_thinking` 默认为 `true`，并通过 `Confirm` 步骤让用户明确确认。这是一个有意的产品决策：wizard 生成的配置显式写入 `enable_thinking = true`，不依赖 serde 默认值。

**替代方案**：保持与 config 默认值一致（默认 false）。放弃原因：对新用户来说 thinking 是核心卖点之一，默认关闭会降低首次体验。

## Risks / Trade-offs

- **[List Models API 不可用]** → 降级到 fallback 硬编码列表 + 显示提示信息。fallback 列表需要随新模型发布手动更新。
- **[toml_edit 与 toml 版本不兼容]** → 两个 crate 由同一 monorepo 维护，版本锁定风险低。需确认 `toml 0.9` 和 `toml_edit` 的 TOML spec 版本一致。
- **[OpenAI 兼容供应商的 List Models]** → 部分兼容服务不支持 `/v1/models`，直接降级为手动输入模型名。
- **[dialoguer 在 Windows Terminal 的兼容性]** → dialoguer 使用 crossterm（krew 已依赖），Windows Terminal 和 Windows Console 均支持。需测试 ConPTY 场景。
- **[配置文件并发写入]** → 单用户 CLI 工具，不存在并发写入场景，无需文件锁。

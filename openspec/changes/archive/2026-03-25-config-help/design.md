## Context

krew-cli 的 agent 在运行时拥有 shell/read/write/edit 等工具，理论上可以帮用户修改配置文件。但当前系统提示词仅提到 "hosted by krew-cli"，agent 不知道 krew 是什么、配置文件结构如何。用户想让 agent 改配置，agent 无从下手。

上一个 change（config-wizard）已经实现了 `krew config init/add/del/list/doctor` 子命令体系。本次在此基础上新增 `krew config help` 并在 identity prompt 中注入提示。

## Goals / Non-Goals

**Goals:**
- 让 agent 在会话中能自主帮助用户修改 krew 配置
- `krew config help` 输出完整、准确的配置手册，与代码中的配置模型完全一致
- identity prompt 中用最少的 token 传递必要信息

**Non-Goals:**
- 不新增专门的 "config tool"——agent 用已有的 shell/edit 工具即可
- 不做动态手册生成（不读取当前配置来生成文档）——纯硬编码静态文本
- 不改变现有 `krew config` 子命令的交互式行为

## Decisions

### D1: 手册内容硬编码在 Rust 代码中

直接在 `help.rs` 中用 `println!` 输出手册文本。

**替代方案**: 从外部文件（如 `docs/CONFIG_MANUAL.md`）读取。
**选择理由**: 配置结构相对稳定，硬编码更简单、无运行时依赖。手册跟代码在一起，改配置字段时更容易同步更新。

### D2: identity prompt 只加两句话

在 `build_identity_prompt()` 中，现有的 "hosted by krew-cli" 之后追加：
1. krew-cli 是什么（一句话描述）
2. 修改配置时可执行 `krew config help`（一句话提示）

**替代方案**: 注入完整的环境能力列表（tools、@mention、slash commands 等）。
**选择理由**: @mention 已有单独的 peer agent 提示注入；tools 通过 tool definitions 注入；slash commands 是用户 TUI 操作，agent 不需要知道。只加配置帮助提示，避免浪费 token。

### D3: `help` 作为 `config` 的子命令而非 clap 内建 help

使用 `krew config help`（自定义子命令），不是 `krew config --help`（clap 自动生成）。

**选择理由**: clap 的 `--help` 只展示参数结构，无法输出我们需要的完整配置手册。自定义子命令可以完全控制输出内容和格式。

### D4: 手册使用英文

手册输出 SHALL 使用英文。因为手册的主要消费者是 agent（LLM），英文是所有模型的最优理解语言，也与代码注释、TOML 字段名保持一致。

### D5: 手册必须准确反映配置层级模型

手册内容 SHALL 与代码中的配置模型完全一致。两层配置文件支持的 section 不同：

- **User config** (`~/.krew/settings.toml`): 支持 `[settings]`（不含 `reply_order`）、`[providers.*]`、`[[mcp_servers]]`、`[skills]`。不支持 `[[agents]]`。
- **Project config** (`.krew/settings.toml`): 支持 `[settings]`（含 `reply_order`）、`[providers.*]`、`[[agents]]`、`[[mcp_servers]]`、`[skills]`。

这与代码中 `UserConfig`（无 agents 字段、`UserSettings` 无 reply_order）和 `RawConfig`（完整结构）的定义一致。

**Merge 规则**:
- providers 按 key 合并（project 覆盖 user 同名 key）
- mcp_servers 按 name 合并（同名用 project 的）
- settings 标量 project `Some` 优先，`None` 继承 user
- skills project `Some` 优先，`None` 继承 user
- agents 和 reply_order 仅存在于 project config

**默认值必须与代码常量一致**: 例如 `compact_keep_rounds` 默认 3、`tools` 默认 true、`worker_threads` 默认 4 等。

### D6: 手册内容大纲

```
=== krew Configuration Manual ===

1. File Locations & Merge Rules
   - User config: ~/.krew/settings.toml
   - Project config: .krew/settings.toml
   - 各层支持的 section
   - Merge 语义

2. Configuration Reference
   - [settings] 完整字段说明（含默认值）
   - [settings.retry] 完整字段说明（含默认值）
   - [providers.<name>] 完整字段说明
   - [[agents]] 完整字段说明（含 sampling 子表、默认值）
   - [[mcp_servers]] 完整字段说明（两种传输模式）
   - [skills] 完整字段说明（含默认值）

3. Example Configurations
   - User config 示例
   - Project config 示例

4. CLI Commands Reference
   - krew config init/add/del/list/doctor/help
```

### D7: 清理过时的 Azure 配置引用

代码中从未实现 `azure_endpoint` / `azure_api_version` 字段。本次 change 已将 `config-types` spec 中的这些字段移除。同时需要清理仓库内其他文档中的过时 Azure 引用：
- `docs/MANUAL_CN.md` 中的 Azure 配置说明
- `docs/TDD.md` 中的 Azure 相关描述

## Risks / Trade-offs

- **[手册过时]** → 测试中直接断言关键字段名和默认值，确保手册内容与代码一致。如果代码修改了默认值但没更新手册，测试会失败。
- **[token 消耗]** → identity prompt 只加两句话（约 40 tokens），影响极小。Agent 按需执行 `krew config help` 获取完整手册，不是每次都注入。

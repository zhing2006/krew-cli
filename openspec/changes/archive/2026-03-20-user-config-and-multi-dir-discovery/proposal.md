## Why

当前配置仅支持 project 级别（`.krew/settings.toml`），用户在多个项目中需要重复配置 providers、API keys、approval_mode 等相同内容。同时 commands 的 discovery 只扫描 `.krew/commands/`，而 skills 已经支持 `.krew/` + `.agents/` 两个目录但缺少 `.claude/` 兼容。这两个不足导致：(1) 跨项目复用配置繁琐；(2) 从 Claude Code 迁移时已有的 `.claude/commands/` 无法直接使用；(3) commands 和 skills 的 discovery 策略不统一。

## What Changes

- 新增 user-level 配置文件 `~/.krew/settings.toml`，支持 `providers`、`settings` 偏好子集、`mcp_servers` 的 user 级定义
- 实现 user config 与 project config 的分层合并：标量字段 project 覆盖 user，providers 同名整项替换，mcp_servers 按 name 去重，skills 整体二选一
- `agents` 和 `reply_order` 仅在 project level 定义，不参与 user-level 合并
- Commands discovery 从单目录扩展为多目录扫描，优先级：`.krew > .agents > .claude`，先 project 后 user
- Skills discovery 新增 `.claude/skills/` 扫描路径，统一为与 commands 相同的 6 路径优先级模式
- 抽取共享的 discovery 路径生成函数，避免路径逻辑重复

## Capabilities

### New Capabilities
- `user-config`: User-level 配置加载与分层合并机制（`~/.krew/settings.toml`）
- `multi-dir-discovery`: Commands 和 Skills 的统一多目录 discovery 路径策略（`.krew > .agents > .claude`，project > user）

### Modified Capabilities
- `config-loading`: 新增 user config 加载与合并逻辑
- `config-types`: Config 结构体需支持 partial/optional 字段用于 user config 合并
- `custom-commands`: discovery 从单目录扩展为多目录
- `skill-discovery`: 新增 `.claude/skills/` 扫描路径

## Impact

- `krew-config` crate: 核心改动，新增 user config 加载、合并逻辑、partial config 类型
- `krew-core` crate: commands discovery 和 skills discovery 路径生成重构
- `krew-cli` crate: `main.rs` 中 config 加载流程调整（先 user 再 project 再合并）
- 文档更新: PDD、TDD、config.example.toml 需补充 user config 说明
- 无破坏性变更：project-only 配置继续正常工作

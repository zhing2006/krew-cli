## Context

当前所有 5 个内建文件工具（read_file、write_file、edit_file、glob、grep）通过 `validate_path()` 函数（`krew-tools/src/lib.rs`）强制检查路径是否在 workspace（cwd）边界内。write_file 使用内联的边界检查逻辑而非 `validate_path`，但效果相同。glob 工具额外在遍历结果中也做了 `cwd_canonical` 前缀检查。

工具注册在 `krew-tools/src/builtin/mod.rs` 的 `create_full_registry()` 和 `create_readonly_registry()` 中完成，由 `krew-core/src/agent/init.rs` 调用，传入 cwd 路径。配置值（如 `approval_mode`）通过 `AgentRuntime` 结构体传递，而非通过工具构造函数。

配置加载走分层合并链路：`UserConfig::load()` → `RawConfig::load()` → `RawConfig::merge_user()` → `RawConfig::resolve()` → 最终 `Config`。其中 `RawSettings`（project 级）和 `UserSettings`（user 级）以 `Option<T>` 保留字段存在性，`merge_user` 通过 `merge_option!` 宏实现 project-优先-user-补充的合并语义，`resolve` 将所有 `Option` 填充为默认值。新增的 `restrict_workspace` 字段必须在这条链路的每个环节都添加。

## Goals / Non-Goals

**Goals:**
- 新增 `restrict_workspace` 布尔配置项（默认 `true`），关闭后所有文件工具可访问系统上任意路径
- 改动最小化：仅修改 `validate_path` 签名和 5 个工具的调用处，不改变工具架构
- 同步更新所有相关文档和配置示例

**Non-Goals:**
- 不做读写分离的细粒度控制（不区分只读/读写的不同放开策略）
- 不做路径白名单功能
- 不修改 shell 工具的行为（shell 工具不受 workspace 边界限制）
- 不修改 MCP 工具的行为

## Decisions

### 1. 通过 `validate_path` 参数传递开关

**方案**: 给 `validate_path` 添加 `restrict: bool` 参数。当 `restrict = false` 时，跳过 `starts_with(cwd_canonical)` 检查，仅执行路径解析（`dunce::canonicalize`）。

**替代方案**: 通过 `ToolContext` 传递。这需要修改 `ToolContext` 结构体并影响所有工具（包括不需要此功能的 shell、fetch_url 等），改动面更大。

**理由**: `validate_path` 是唯一的边界检查点（write_file 除外），直接在此处加开关最干净。每个工具已经持有 `cwd`，只需额外持有 `restrict_workspace` 即可。

### 2. 工具构造函数传入 `restrict_workspace`

**方案**: 修改 5 个文件工具的 `new()` 方法，增加 `restrict_workspace: bool` 参数。`create_full_registry` 和 `create_readonly_registry` 也相应增加参数，由 `init.rs` 从 `config.settings.restrict_workspace` 传入。

**理由**: 与 `cwd` 传递方式一致，简单直接。

### 3. write_file 的内联边界检查统一为调用 `validate_path`

**方案**: 不统一。write_file 的内联检查在 `dunce::canonicalize` 之前就需要 `normalize_path`（因为目标文件可能还不存在），与 `validate_path` 的逻辑不同。直接在内联检查处加 `restrict_workspace` 条件跳过即可。

### 4. glob 工具的双重检查

glob 有两处边界检查：`validate_path`（检查 search_dir）和遍历时的 `cwd_canonical` 前缀过滤。当 `restrict_workspace = false` 时，两处都需要跳过。

## Risks / Trade-offs

- **安全性降低** → 默认值为 `true`，用户必须主动配置才会关闭；配合 `approval_mode` 可提供二次确认
- **AI 误操作** → `restrict_workspace = false` 时 AI 可能写入/删除任意文件 → 建议文档中提醒用户配合 `approval_mode = "suggest"` 使用

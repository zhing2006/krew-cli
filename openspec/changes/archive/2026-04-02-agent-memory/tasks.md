## 1. Memory 模块核心

- [x] 1.1 在 `krew-core/src/` 下新建 `memory/` 模块（`mod.rs`），定义 `load_memory_prompt(agent_name: &str, cwd: &str, has_tools: bool) -> Option<String>` 公开函数
- [x] 1.2 实现 `read_and_truncate(path, max_lines, max_bytes)` 函数：读取文件内容，按行数（200）和字节数（25,000）截断，超出时附加警告信息
- [x] 1.3 实现目录自动创建逻辑：在 `load_memory_prompt()` 中调用 `create_dir_all` 创建 `.krew/memory/` 和 `.krew/memory/agents/{agent_name}/` 目录，失败时静默返回 None
- [x] 1.4 定义 Memory 指令模板常量（`MEMORY_PROMPT_TEMPLATE`）：包含两层存储说明、四种记忆类型定义（含 scope 标注）、不应保存的内容、保存流程、访问指导、大小限制
- [x] 1.5 实现 `is_memory_path(file_path: &str, cwd: &str) -> bool` 函数：判断路径是否在 `.krew/memory/` 下（供 approval carve-out 使用）

## 2. Memory Prompt 构建

- [x] 2.1 实现 `load_memory_prompt()` 完整逻辑：当 `has_tools=true` 时注入完整模板（含写入指令）+ 索引内容；当 `has_tools=false` 时仅注入索引内容
- [x] 2.2 模板变量替换：将 `{{agent_name}}` 替换为当前 Agent 的 name
- [x] 2.3 加载 Global MEMORY.md（`.krew/memory/MEMORY.md`）→ 截断 → 添加 `## Global Memory` 标题
- [x] 2.4 加载 Per-Agent MEMORY.md（`.krew/memory/agents/{agent_name}/MEMORY.md`）→ 截断 → 添加 `## Your Memory` 标题
- [x] 2.5 处理边界情况：MEMORY.md 不存在时跳过对应段落、内容为空时跳过、目录创建失败时返回 None

## 3. Approval Carve-out

- [x] 3.1 在 `krew-core/src/agent/approval.rs` 的 `check_tool_approval()` 函数中，Step 0（deny rules）之后、Step 1（bypass immunity）之前，添加 `.krew/memory/**` 路径的 carve-out：匹配时返回 `ToolApproval::Approved`
- [x] 3.2 carve-out 逻辑对 `read_file`、`write_file`、`edit_file` 三种工具生效

## 4. System Prompt 集成

- [x] 4.1 修改 `krew-core/src/agent/mod.rs` 中的 `build_system_prompt()` 函数：在 Sub-Agent Catalog 之后、Agent Prompt 之前调用 `load_memory_prompt()` 并拼接结果
- [x] 4.2 将 `cwd` 和 `has_tools`（来自 `config.tools`）信息传入 `build_system_prompt()` 调用链

## 5. 测试

- [x] 5.1 单元测试 `read_and_truncate()`：正常内容、超行截断、超字节截断、文件不存在
- [x] 5.2 单元测试 `load_memory_prompt()`：无 memory 目录、仅 Global MEMORY.md、仅 Per-Agent MEMORY.md、两者都有、内容为空、has_tools=false 时不含写入指令
- [x] 5.3 单元测试 `is_memory_path()`：匹配 `.krew/memory/` 子路径、不匹配 `.krew/settings.toml`、Windows 反斜杠处理
- [x] 5.4 单元测试 approval carve-out：memory 路径自动放行、非 memory 的 .krew 路径仍需审批、deny_rules 优先于 carve-out

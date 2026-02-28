## 1. krew-config: 常量与加载函数

- [x] 1.1 在 `krew-config/src/lib.rs` 中定义 `PROJECT_INSTRUCTIONS_FILENAME` 常量（值 `"AGENTS.md"`）和 `PROJECT_INSTRUCTIONS_MAX_SIZE` 常量（值 `102400`）
- [x] 1.2 新建 `krew-config/src/instructions.rs` 模块，实现 `load_project_instructions(cwd: &Path) -> Result<Option<String>>` 函数：从 cwd 向上遍历查找 `AGENTS.md`，按祖先在前子目录在后的顺序合并内容，处理大小限制（100KB 截断）和非 UTF-8 文件（跳过并记录警告）
- [x] 1.3 在 `krew-config/src/lib.rs` 中 `pub mod instructions` 并重导出 `load_project_instructions`
- [x] 1.4 为 `load_project_instructions` 编写单元测试：无文件返回 None、单文件加载、多层级合并顺序、大文件截断

## 2. krew-core: 系统提示词注入

- [x] 2.1 在 `krew-core` 的 Agent 系统消息构建逻辑中，增加 `project_instructions: Option<String>` 参数，拼接到 `system_prompt` 之前（使用 `<project-instructions>` 标签包裹）
- [x] 2.2 为系统消息拼接逻辑编写单元测试：有指令+有 system_prompt、有指令+无 system_prompt、无指令三种场景

## 3. krew-cli: App 初始化集成

- [x] 3.1 在 `krew-cli` 的 App 初始化流程中，调用 `load_project_instructions(cwd)` 加载指令内容，存入运行时状态并传递给 Agent 构建流程

## 4. 文档更新

- [x] 4.1 更新 PDD §4.6 配置系统：新增 §4.6.3 项目级指令文件，说明 `AGENTS.md` 的用途、位置、层级加载规则
- [x] 4.2 更新 TDD §3.7 配置管理：新增指令文件加载的技术设计（函数签名、遍历策略、大小限制、注入格式）

## ADDED Requirements

### Requirement: PermissionRule 结构体
`krew-config` SHALL 定义 `PermissionRule` 结构体，包含字段：`tool: String`（工具名）、`pattern: Option<String>`（匹配模式）、`reason: Option<String>`（拒绝/确认原因）。该结构体 SHALL 派生 `Deserialize`、`Clone`、`Debug`。

#### Scenario: 完整规则反序列化
- **WHEN** TOML 包含 `[[deny_rules]]` 且含 `tool = "shell"`、`pattern = "rm -rf *"`、`reason = "禁止递归强制删除"`
- **THEN** SHALL 反序列化为 `PermissionRule { tool: "shell", pattern: Some("rm -rf *"), reason: Some("禁止递归强制删除") }`

#### Scenario: 仅 tool 字段
- **WHEN** TOML 包含 `[[deny_rules]]` 且仅含 `tool = "fetch_url"`
- **THEN** SHALL 反序列化为 `PermissionRule { tool: "fetch_url", pattern: None, reason: None }`，表示整个工具被匹配

### Requirement: 规则匹配函数
`krew-core` SHALL 实现 `matches_rule(tool_name: &str, arguments: &str, rule: &PermissionRule) -> bool` 函数，根据工具类型选择匹配策略：

- 若 `rule.tool` 与 `tool_name` 不匹配，SHALL 返回 `false`
- 若 `rule.pattern` 为 `None`，SHALL 匹配该工具的所有调用
- 若 `tool_name` 为 `"shell"`，SHALL 复用 `shell_parse.rs` 的命令段拆分逻辑，对复合命令的每个独立命令段分别进行通配符匹配
- 若 `tool_name` 为 `"write_file"`、`"edit_file"` 或 `"read_file"`，SHALL 先对文件路径进行归一化，再使用 glob 模式匹配
- 若 `tool_name` 为 `"fetch_url"`，SHALL 使用域名后缀匹配 URL host
- 其他工具有 pattern 时 SHALL 忽略 pattern（视为整工具匹配）

#### Scenario: shell 通配符匹配
- **WHEN** 规则为 `tool = "shell"`, `pattern = "cargo build *"` 且工具调用为 `shell("cargo build --release")`
- **THEN** SHALL 返回 `true`

#### Scenario: shell 通配符不匹配
- **WHEN** 规则为 `tool = "shell"`, `pattern = "cargo build *"` 且工具调用为 `shell("cargo test")`
- **THEN** SHALL 返回 `false`

#### Scenario: shell 通配符尾部可选
- **WHEN** 规则为 `tool = "shell"`, `pattern = "cargo *"` 且工具调用为 `shell("cargo")`
- **THEN** SHALL 返回 `true`（尾部 ` *` 使参数可选）

#### Scenario: 文件路径 glob 匹配
- **WHEN** 规则为 `tool = "write_file"`, `pattern = ".git/**"` 且工具调用目标路径为 `.git/config`
- **THEN** SHALL 返回 `true`

#### Scenario: 文件路径 glob 不匹配
- **WHEN** 规则为 `tool = "write_file"`, `pattern = "src/**"` 且工具调用目标路径为 `tests/foo.rs`
- **THEN** SHALL 返回 `false`

#### Scenario: read_file glob 匹配
- **WHEN** 规则为 `tool = "read_file"`, `pattern = ".env"` 且工具调用目标路径为 `.env`
- **THEN** SHALL 返回 `true`

#### Scenario: fetch_url 域名后缀匹配
- **WHEN** 规则为 `tool = "fetch_url"`, `pattern = "github.com"` 且工具调用 URL host 为 `api.github.com`
- **THEN** SHALL 返回 `true`

#### Scenario: fetch_url 域名不匹配
- **WHEN** 规则为 `tool = "fetch_url"`, `pattern = "github.com"` 且工具调用 URL host 为 `gitlab.com`
- **THEN** SHALL 返回 `false`

#### Scenario: 无 pattern 匹配整个工具
- **WHEN** 规则为 `tool = "shell"`, `pattern = None` 且工具调用为任意 shell 命令
- **THEN** SHALL 返回 `true`

#### Scenario: 工具名不匹配
- **WHEN** 规则为 `tool = "shell"` 且工具调用为 `write_file`
- **THEN** SHALL 返回 `false`

### Requirement: Shell 复合命令的规则匹配
对于包含 `&&`、`||`、`;`、`|` 的复合 shell 命令，规则匹配 SHALL 复用 `shell_parse.rs` 中的 `extract_command_prefixes()` / `split_shell_operators()` 逻辑，将命令拆分为独立命令段后对每段分别匹配：

- **deny 规则**：任意一个命令段匹配即整条命令被拒绝
- **allow 规则**：所有命令段都必须匹配才放行
- **ask 规则**：任意一个命令段匹配即整条命令需要确认

当命令包含复杂构造（`$()`、反引号、重定向 `>`/`<` 等）导致 `extract_command_prefixes()` 返回 `None` 时，三类规则的回退行为：
- **deny** 规则 SHALL 对原始整条命令字符串做通配符匹配（尽力拦截）
- **ask** 规则 SHALL 对原始整条命令字符串做通配符匹配（尽力确认）
- **allow** 规则 SHALL 不生效（回退到需要审批，不冒险放行）

#### Scenario: deny 匹配复合命令的部分段
- **WHEN** deny 规则为 `tool = "shell"`, `pattern = "rm *"` 且命令为 `git status && rm -rf /tmp`
- **THEN** SHALL 返回 `true`（第二段 `rm -rf /tmp` 匹配 deny 规则）

#### Scenario: allow 不放行部分段匹配
- **WHEN** allow 规则为 `tool = "shell"`, `pattern = "git status *"` 且命令为 `git status && rm -rf /`
- **THEN** SHALL 返回 `false`（allow 需要所有段都匹配，第二段 `rm -rf /` 不匹配）

#### Scenario: 复杂构造下 allow 不生效
- **WHEN** allow 规则为 `tool = "shell"`, `pattern = "echo *"` 且命令为 `echo $(rm -rf /)`
- **THEN** SHALL 返回 `false`（复杂构造，allow 不生效）

#### Scenario: 复杂构造下 deny 整串匹配
- **WHEN** deny 规则为 `tool = "shell"`, `pattern = "*rm -rf*"` 且命令为 `echo $(rm -rf /)`
- **THEN** SHALL 返回 `true`（复杂构造，deny 对整串做通配符匹配）

#### Scenario: 复杂构造下 ask 整串匹配
- **WHEN** ask 规则为 `tool = "shell"`, `pattern = "*publish*"` 且命令为 `echo $(npm publish)`
- **THEN** SHALL 返回 `true`（复杂构造，ask 对整串做通配符匹配，触发确认）

### Requirement: Shell 通配符匹配算法
Shell 命令的通配符匹配 SHALL 实现以下语义：
- `*` 匹配任意字符序列（转为正则 `.*`）
- `\*` 匹配字面量星号
- `\\` 匹配字面量反斜杠
- 匹配 SHALL 为全字符串匹配（锚定 `^...$`）
- 当 pattern 以 ` *` 结尾且仅含一个未转义 `*` 时，尾部空格和参数 SHALL 变为可选

#### Scenario: 基本通配符
- **WHEN** pattern 为 `npm install *` 且命令为 `npm install express`
- **THEN** SHALL 匹配

#### Scenario: 转义星号
- **WHEN** pattern 为 `echo \*` 且命令为 `echo *`
- **THEN** SHALL 匹配

#### Scenario: 尾部可选参数
- **WHEN** pattern 为 `git status *` 且命令为 `git status`
- **THEN** SHALL 匹配（尾部 ` *` 可选）

#### Scenario: 多个通配符
- **WHEN** pattern 为 `docker * build *` 且命令为 `docker -f Dockerfile build .`
- **THEN** SHALL 匹配

### Requirement: 文件路径归一化
在进行 glob 匹配之前，SHALL 对工具参数中的文件路径执行以下归一化步骤：
1. Windows 反斜杠转为正斜杠
2. 绝对路径转为相对于 cwd 的相对路径
3. 规范化 `..` 和 `.` 路径段
4. 去除前导 `./`

归一化后的路径用于所有规则匹配和 bypass 免疫检查，确保规则行为一致。

#### Scenario: 绝对路径归一化
- **WHEN** cwd 为 `/home/user/project` 且文件路径为 `/home/user/project/.env`
- **THEN** 归一化后 SHALL 为 `.env`

#### Scenario: 相对路径带 ..
- **WHEN** 文件路径为 `src/../.env`
- **THEN** 归一化后 SHALL 为 `.env`

#### Scenario: 前导 ./ 去除
- **WHEN** 文件路径为 `./.git/config`
- **THEN** 归一化后 SHALL 为 `.git/config`

#### Scenario: Windows 绝对路径
- **WHEN** cwd 为 `G:\AI\Work\project` 且文件路径为 `G:\AI\Work\project\src\main.rs`
- **THEN** 归一化后 SHALL 为 `src/main.rs`

### Requirement: 文件路径 glob 匹配算法
文件路径的 glob 匹配 SHALL 在归一化后的路径上执行，实现以下语义：
- `*` 匹配单层目录中的任意文件名
- `**` 匹配任意深度的目录
- 匹配基于归一化后的相对路径

#### Scenario: 双星号递归匹配
- **WHEN** pattern 为 `src/**` 且归一化后文件路径为 `src/core/mod.rs`
- **THEN** SHALL 匹配

#### Scenario: 单星号不递归
- **WHEN** pattern 为 `src/*` 且归一化后文件路径为 `src/core/mod.rs`
- **THEN** SHALL 不匹配（`*` 不跨目录）

#### Scenario: 精确文件名匹配
- **WHEN** pattern 为 `.env` 且归一化后文件路径为 `.env`
- **THEN** SHALL 匹配

#### Scenario: Windows 原始路径经归一化后匹配
- **WHEN** pattern 为 `src/**` 且原始文件路径为 `src\core\mod.rs`（Windows 反斜杠）
- **THEN** SHALL 匹配（归一化已转为 POSIX 格式）

### Requirement: 三维规则优先级
当同一个工具调用同时匹配 deny、ask 和 allow 规则时，SHALL 按以下优先级执行：
1. **deny** 优先级最高：匹配到立即返回 `Denied`，不检查后续规则
2. **ask** 次之：匹配到返回 `NeedsApproval`，bypass 免疫
3. **allow** 最低：仅在 deny 和 ask 都未匹配时才放行

#### Scenario: deny 覆盖 allow
- **WHEN** `deny_rules` 包含 `tool = "shell", pattern = "rm *"` 且 `allow_rules` 包含 `tool = "shell"`
- **AND** 工具调用为 `shell("rm foo.txt")`
- **THEN** SHALL 返回 `Denied`（deny 优先）

#### Scenario: ask 覆盖 allow
- **WHEN** `ask_rules` 包含 `tool = "shell", pattern = "npm publish *"` 且 `allow_rules` 包含 `tool = "shell", pattern = "npm *"`
- **AND** 工具调用为 `shell("npm publish")`
- **THEN** SHALL 返回 `NeedsApproval`（ask 优先）

#### Scenario: ask 不覆盖 deny
- **WHEN** `deny_rules` 包含 `tool = "shell", pattern = "rm -rf *"` 且 `ask_rules` 包含 `tool = "shell", pattern = "rm *"`
- **AND** 工具调用为 `shell("rm -rf /")`
- **THEN** SHALL 返回 `Denied`（deny 优先于 ask）

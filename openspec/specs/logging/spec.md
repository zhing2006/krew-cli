## ADDED Requirements

### Requirement: 日志初始化
`krew-cli` SHALL 在启动时初始化 tracing 日志系统，使用 `tracing-subscriber` 和 `tracing-appender` 将日志写入 `.krew/logs/` 目录下的文件。日志 SHALL 按天滚动（daily rolling），每天生成一个新的日志文件。日志 SHALL NOT 输出到 stdout 或 stderr。

#### Scenario: 启动后日志文件创建
- **WHEN** 用户执行 `krew`
- **THEN** 系统 SHALL 在当前目录的 `.krew/logs/` 下创建当天的日志文件

#### Scenario: 日志不干扰 TUI
- **WHEN** 程序运行期间产生 tracing 日志
- **THEN** 日志内容 SHALL 仅写入文件，终端界面 SHALL NOT 显示任何日志输出

#### Scenario: 每日滚动
- **WHEN** 跨天运行或新一天启动 krew
- **THEN** 系统 SHALL 创建新的日志文件，不覆盖之前的日志

### Requirement: 日志保留策略
系统 SHALL 在启动时清理过期的日志文件。保留天数默认为 7 天，后续可通过配置文件覆盖。超过保留天数的日志文件 SHALL 被自动删除。

#### Scenario: 默认保留 7 天
- **WHEN** 未配置日志保留天数
- **THEN** 系统 SHALL 保留最近 7 天的日志文件，删除更早的日志文件

#### Scenario: 不足 7 天时不删除
- **WHEN** `.krew/logs/` 中所有日志文件均在 7 天内
- **THEN** 系统 SHALL NOT 删除任何日志文件

### Requirement: 日志目录自动创建
系统 SHALL 在日志初始化前自动创建 `.krew/logs/` 目录（包含父目录 `.krew/`）。若目录已存在，SHALL 正常继续。

#### Scenario: 目录不存在时自动创建
- **WHEN** `.krew/logs/` 目录不存在
- **THEN** 系统 SHALL 自动创建该目录并成功初始化日志

#### Scenario: 目录已存在时不报错
- **WHEN** `.krew/logs/` 目录已存在
- **THEN** 系统 SHALL 正常初始化日志，不产生错误

### Requirement: verbose 控制日志级别
当 CLI 传入 `--verbose` 参数时，日志级别 SHALL 设为 `DEBUG`；否则 SHALL 设为 `INFO`。

#### Scenario: 默认日志级别
- **WHEN** 用户执行 `krew`（不带 `--verbose`）
- **THEN** 日志系统 SHALL 使用 `INFO` 级别

#### Scenario: verbose 模式
- **WHEN** 用户执行 `krew --verbose`
- **THEN** 日志系统 SHALL 使用 `DEBUG` 级别

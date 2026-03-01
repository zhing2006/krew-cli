## 1. krew-core: ProcessStats 模块

- [x] 1.1 创建 `crates/krew-core/src/process_stats.rs`，定义 `ProcessStats` 结构体和 `collect()` / `format_memory()` 方法签名
- [x] 1.2 实现 Linux 平台：读取 `/proc/self/status` 解析 `VmRSS` 和 `Threads`
- [x] 1.3 实现 Windows 平台：`GetProcessMemoryInfo` 获取内存，`CreateToolhelp32Snapshot` 获取线程数
- [x] 1.4 实现 macOS 平台：`mach_task_basic_info` 获取内存，`proc_pidinfo` 获取线程数
- [x] 1.5 实现 `format_memory()` 方法（B/KB/MB/GB 自动换算）
- [x] 1.6 在 `crates/krew-core/src/lib.rs` 中导出 `process_stats` 模块

## 2. krew-core: SlashCommand 扩展

- [x] 2.1 在 `command.rs` 的 `SlashCommand` 枚举中添加 `Stats` 变体
- [x] 2.2 在 `from_input()` 中添加 `/stats` 解析
- [x] 2.3 在 `name()` 和 `description()` 中添加对应条目
- [x] 2.4 在 `all_help()` 中添加 `/stats` 帮助信息

## 3. krew-cli: /stats 命令执行

- [x] 3.1 在 `commands.rs` 的 `execute_slash_command()` 中添加 `Stats` 分支
- [x] 3.2 实现 `execute_stats()` 方法，调用 `ProcessStats::collect()` 并格式化为 ratatui Lines/Spans 输出

## 4. 验证

- [x] 4.1 运行 `cargo fmt --all` 和 `cargo clippy --all-targets --all-features -- -D warnings`
- [x] 4.2 运行 `cargo test` 确认无回归
- [x] 4.3 运行 `cargo run` 手动测试 `/stats` 命令输出

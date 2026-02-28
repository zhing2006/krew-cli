## 1. 依赖与项目配置

- [x] 1.1 在根 `Cargo.toml` 的 `[workspace.dependencies]` 中添加 `tracing-appender`、`ratatui-textarea`、`crossterm` 依赖
- [x] 1.2 在 `crates/krew-cli/Cargo.toml` 中引入 `tracing-appender`、`ratatui-textarea`、`crossterm` 依赖

## 2. 日志系统

- [x] 2.1 在 `main.rs` 中实现日志初始化函数 `init_logging(verbose: bool)`：自动创建 `.krew/logs/` 目录，使用 `tracing-appender::rolling::daily` 按天滚动写入日志文件，根据 `verbose` 设置 `DEBUG` 或 `INFO` 级别
- [x] 2.2 实现日志清理：启动时扫描 `.krew/logs/` 目录，删除超过保留天数（默认 7 天）的日志文件
- [x] 2.3 在 `main()` 中解析 CLI 参数后调用 `init_logging(cli.verbose)`，验证日志文件生成和清理

## 3. TUI 基础框架

- [x] 3.1 在 `app.rs` 中重构 `App` 结构体：添加消息列表 `messages: Vec<ChatMessage>`、`ratatui-textarea` 输入组件、滚动偏移等 TUI 状态字段
- [x] 3.2 在 `main.rs` 中实现 TUI 终端初始化：进入 raw mode、alternate screen，启用 keyboard enhancement flags，创建 ratatui Terminal
- [x] 3.3 在 `main.rs` 中实现 TUI 终端清理：离开 alternate screen、关闭 raw mode，使用 panic hook 确保异常时也能恢复终端
- [x] 3.4 将 `main()` 改为 `#[tokio::main] async fn main()`，在终端初始化和清理之间运行应用主循环

## 4. 事件循环与输入处理

- [x] 4.1 在 `app.rs` 中实现异步主事件循环 `App::run()`：使用 `crossterm::event::EventStream` + `tokio::select!` 监听终端事件
- [x] 4.2 实现键盘事件处理：Enter 发送消息、Shift+Enter 换行、Page Up/Down 滚动、鼠标滚轮滚动（需启用 crossterm 鼠标捕获）
- [x] 4.3 实现双击 Ctrl+C 退出：第一次 Ctrl+C 显示 "Press Ctrl+C again to quit" 提示并启动 2 秒计时，2 秒内再次按下 Ctrl+C 退出，超时后提示消失并重置状态（参考 Codex `QUIT_SHORTCUT_TIMEOUT` 模式）
- [x] 4.4 实现 `/quit` 命令检测：输入文本为 `/quit` 时退出程序

## 5. 渲染

- [x] 5.1 在 `render.rs` 中实现 `render(frame, app)` 函数：使用 `Layout` 将界面分为上方输出区域和下方输入区域
- [x] 5.2 实现输出区域渲染：显示消息列表（`you>` 前缀的用户消息和 `echo:` 前缀的回复），支持滚动
- [x] 5.3 实现输入区域渲染：渲染 `ratatui-textarea` 组件，显示 `you>` 提示符
- [x] 5.4 实现 ASCII banner：启动时在输出区域显示 PDD §5.1 定义的 krew logo 和版本信息

## 6. Echo 模式

- [x] 6.1 在 `app.rs` 中实现消息发送逻辑：Enter 发送时将用户输入添加为 `you>` 消息，然后将相同内容添加为 `echo:` 回复消息
- [x] 6.2 处理空输入：输入为空或仅含空白字符时不产生任何输出

## 7. 集成与验证

- [x] 7.1 确保 AGENTS.md 加载逻辑（现有 `App::new()` 中的功能）在新架构中保留
- [x] 7.2 运行 `cargo fmt --all` 和 `cargo clippy --all-targets --all-features -- -D warnings` 确保代码质量
- [x] 7.3 运行 `cargo run` 端到端验证：启动显示 banner → 输入文本 → echo 回显 → `/quit` 退出

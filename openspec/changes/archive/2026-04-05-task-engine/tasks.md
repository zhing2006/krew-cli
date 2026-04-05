## 1. Task 模块核心实现

- [x] 1.1 创建 `krew-core/src/task/types.rs`，定义 `TaskRequest` 和 `TaskResult` 结构体
- [x] 1.2 创建 `krew-core/src/task/mod.rs`，实现 `run_task()` 和 `run_task_with_events()` 函数
- [x] 1.3 在 `krew-core/src/lib.rs` 中添加 `pub mod task;`

## 2. 验证

- [x] 2.1 确保 `cargo check --all-targets` 通过
- [x] 2.2 确保 `cargo clippy --all-targets --all-features -- -D warnings` 通过
- [x] 2.3 确保 `cargo test -p krew-core` 全部通过（现有测试无回归）

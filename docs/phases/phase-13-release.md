# Phase 13: 静态链接 + 发布

> 目标：五平台构建单文件可执行程序，可通过 GitHub Release 和 npm 分发。
> 状态：✅ 已完成

## 实现内容

- **Windows x64**：`static_vcruntime` 静态链接 MSVC 运行时
- **Linux x64/arm64**：`x86_64/aarch64-unknown-linux-musl` + `mimalloc` 替代默认分配器
- **macOS x64/arm64**：`RUSTFLAGS="-C target-feature=+crt-static"`
- **CI/CD**：GitHub Actions 五平台构建 + Release 发布（tag 触发）
- **二进制优化**：strip、LTO、codegen-units=1、panic=abort
- **npm 分发**：`@zhing2026/krew` 主包 + 5 个平台子包（optionalDependencies 模式）

## 验收标准

- 五平台各产出一个单文件可执行程序
- Windows 上无需安装 VC 运行时
- Linux 上无动态库依赖（`ldd` 显示 `not a dynamic executable`）
- `npm install -g @zhing2026/krew` 可安装并运行

## 参考

| 文档 | 位置 | 内容 |
| ---- | ---- | ---- |
| TDD | L75-83 | §2.2 三平台静态链接策略 |
| TDD | L107-108 | static_vcruntime / mimalloc 选型 |
| PDD | L543-549 | §7.4 兼容性要求 |

# Phase 13: 静态链接 + 发布

> 目标：三平台构建单文件可执行程序，可分发。

## 实现内容

- **Windows**：`static_vcruntime` 静态链接 MSVC 运行时
- **Linux**：`x86_64-unknown-linux-musl` + `mimalloc` 替代默认分配器
- **macOS**：`RUSTFLAGS="-C target-feature=+crt-static"`
- **CI/CD**：GitHub Actions 三平台构建 + Release 发布
- **二进制优化**：strip、LTO、codegen-units=1

## 验收标准

- 三平台各产出一个单文件可执行程序
- Windows 上无需安装 VC 运行时
- Linux 上无动态库依赖（`ldd` 显示 `not a dynamic executable`）

## 参考

| 文档 | 位置 | 内容 |
| ---- | ---- | ---- |
| TDD | L75-83 | §2.2 三平台静态链接策略 |
| TDD | L107-108 | static_vcruntime / mimalloc 选型 |
| PDD | L543-549 | §7.4 兼容性要求 |

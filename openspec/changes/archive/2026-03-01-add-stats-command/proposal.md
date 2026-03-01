## Why

用户在调试和性能分析时，需要了解 krew-cli 进程的运行时资源占用情况（内存、线程数）。目前没有内建方式查看这些信息，只能依赖外部工具（如任务管理器）。添加 `/stats` 命令可以在 CLI 内直接查看，方便快速诊断。

## What Changes

- 在 krew-core 中新增跨平台进程状态查询模块，使用原生系统 API（无额外第三方依赖）获取当前进程的内存占用和线程数
- 在 krew-cli 中新增 `/stats` Slash 命令，以格式化方式展示进程运行时信息
- 支持 Windows、Linux、macOS 三平台

## Capabilities

### New Capabilities
- `process-stats`: 跨平台进程运行时状态查询（内存占用、线程数），使用原生系统 API 实现

### Modified Capabilities
- `slash-commands`: 新增 `/stats` 命令的识别与执行

## Impact

- **代码**: krew-core 新增 `process_stats` 模块；krew-core `command.rs` 新增 `Stats` 变体；krew-cli `commands.rs` 新增执行逻辑
- **依赖**: 无新增第三方依赖，使用 `libc`（已有）和原生 FFI 调用
- **API**: 无外部 API 变更

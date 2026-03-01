## Context

krew-cli 目前没有运行时自检能力。用户需要借助外部工具查看进程资源占用。项目已有 `libc` 依赖（用于 Linux/macOS），Windows 平台可通过 `extern "system"` 裸 FFI 调用 Win32 API，无需额外依赖。

现有 Slash 命令体系（`SlashCommand` 枚举 + `execute_slash_command` 分发）已成熟，新增命令只需追加变体和执行逻辑。

## Goals / Non-Goals

**Goals:**
- 提供跨平台（Windows/Linux/macOS）的进程内存占用和线程数查询
- 零额外第三方依赖，使用各平台原生 API
- 通过 `/stats` 命令在 TUI 中格式化展示

**Non-Goals:**
- 不做持续监控或实时刷新（只是单次快照）
- 不采集 CPU 使用率、磁盘 IO 等其他指标
- 不做历史趋势记录

## Decisions

### Decision 1: 自写 FFI 而非使用第三方 crate

**选择**: 直接调用各平台原生 API

**理由**:
- `memory-stats` 依赖 `windows-sys 0.52`，与项目现有 `0.60/0.61` 版本冲突，产生 dup
- `sysinfo` 会引入 `windows 0.62` crate（与现有 `windows-sys` 是不同 crate 家族），依赖过重
- 所需 API 极其简单稳定（十几年未变），自写维护成本趋近于零

**各平台实现方式**:

| 平台 | 内存 | 线程数 |
|------|------|--------|
| Linux | 读 `/proc/self/status` 解析 `VmRSS` | 读 `/proc/self/status` 解析 `Threads` |
| Windows | `GetProcessMemoryInfo` (Win32 API) | `CreateToolhelp32Snapshot` + `Thread32First/Next` |
| macOS | `mach_task_basic_info` (mach API) | `proc_pidinfo` (libproc API) |

### Decision 2: 模块放置在 krew-core

**选择**: `krew-core/src/process_stats.rs`

**理由**:
- 进程状态查询是通用能力，不依赖 TUI
- 放在 krew-core 便于测试和未来复用
- 遵循现有架构：core 提供逻辑，cli 负责展示

### Decision 3: 返回值设计

**选择**: 返回 `ProcessStats` 结构体，字段为 `Option` 类型

```
ProcessStats {
    memory_bytes: Option<u64>,    // RSS 物理内存（字节）
    thread_count: Option<u32>,    // 线程数
}
```

**理由**: 某个平台上某个指标获取失败不应阻塞其他指标的展示，用 `Option` 优雅降级。

## Risks / Trade-offs

- **[平台覆盖不全]** → 不支持的平台编译时返回全 `None`，不会编译失败
- **[FFI 安全性]** → 所有 FFI 调用包裹在 `unsafe` 块中，仅使用只读查询 API，无副作用
- **[数值精度]** → 各平台内存单位不同（Linux 为 kB，Windows 为字节，macOS 为字节），统一转换为字节后展示

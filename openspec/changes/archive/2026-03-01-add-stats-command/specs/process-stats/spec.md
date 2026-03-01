## ADDED Requirements

### Requirement: 进程内存查询
系统 SHALL 提供 `ProcessStats::collect()` 函数，返回当前进程的物理内存占用（RSS）。内存值 SHALL 以字节为单位返回。如果当前平台不支持或查询失败，SHALL 返回 `None` 而非报错。

#### Scenario: Linux 内存查询
- **WHEN** 在 Linux 平台调用 `ProcessStats::collect()`
- **THEN** 系统 SHALL 读取 `/proc/self/status` 文件，解析 `VmRSS` 字段，将 kB 值转换为字节后返回

#### Scenario: Windows 内存查询
- **WHEN** 在 Windows 平台调用 `ProcessStats::collect()`
- **THEN** 系统 SHALL 调用 `GetProcessMemoryInfo` API，返回 `WorkingSetSize` 值（字节）

#### Scenario: macOS 内存查询
- **WHEN** 在 macOS 平台调用 `ProcessStats::collect()`
- **THEN** 系统 SHALL 通过 `mach_task_basic_info` 获取 `resident_size` 值（字节）

#### Scenario: 查询失败
- **WHEN** 系统 API 调用失败或文件读取失败
- **THEN** `memory_bytes` 字段 SHALL 为 `None`

### Requirement: 进程线程数查询
系统 SHALL 提供当前进程的线程数量。如果当前平台不支持或查询失败，SHALL 返回 `None` 而非报错。

#### Scenario: Linux 线程数查询
- **WHEN** 在 Linux 平台调用 `ProcessStats::collect()`
- **THEN** 系统 SHALL 读取 `/proc/self/status` 文件，解析 `Threads` 字段并返回

#### Scenario: Windows 线程数查询
- **WHEN** 在 Windows 平台调用 `ProcessStats::collect()`
- **THEN** 系统 SHALL 使用 `CreateToolhelp32Snapshot` 遍历当前进程的线程并计数

#### Scenario: macOS 线程数查询
- **WHEN** 在 macOS 平台调用 `ProcessStats::collect()`
- **THEN** 系统 SHALL 通过 `proc_pidinfo` 获取线程数量

#### Scenario: 查询失败
- **WHEN** 系统 API 调用失败
- **THEN** `thread_count` 字段 SHALL 为 `None`

### Requirement: ProcessStats 结构体
`ProcessStats` SHALL 包含以下字段：
- `memory_bytes: Option<u64>` — 物理内存占用（字节）
- `thread_count: Option<u32>` — 线程数量

#### Scenario: 结构体实例化
- **WHEN** 调用 `ProcessStats::collect()`
- **THEN** SHALL 返回包含当前进程内存和线程数的 `ProcessStats` 实例

### Requirement: 内存格式化显示
`ProcessStats` SHALL 提供 `format_memory()` 方法，将字节数转换为人类可读格式。

#### Scenario: 小于 1 KB
- **WHEN** `memory_bytes` 为 `Some(512)`
- **THEN** `format_memory()` SHALL 返回 `"512 B"`

#### Scenario: MB 级别
- **WHEN** `memory_bytes` 为 `Some(15_728_640)`（15 MB）
- **THEN** `format_memory()` SHALL 返回 `"15.00 MB"`

#### Scenario: 无数据
- **WHEN** `memory_bytes` 为 `None`
- **THEN** `format_memory()` SHALL 返回 `"N/A"`

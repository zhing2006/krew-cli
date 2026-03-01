//! Cross-platform process statistics (memory usage, thread count).
//!
//! Uses native OS APIs with no third-party dependencies:
//! - Linux: reads `/proc/self/status`
//! - Windows: Win32 API (`GetProcessMemoryInfo`, `CreateToolhelp32Snapshot`)
//! - macOS: mach / libproc APIs

/// Snapshot of current process resource usage.
#[derive(Debug, Clone)]
pub struct ProcessStats {
    /// Resident physical memory in bytes, or `None` if unavailable.
    pub memory_bytes: Option<u64>,
    /// Number of threads in the current process, or `None` if unavailable.
    pub thread_count: Option<u32>,
}

impl ProcessStats {
    /// Collect process statistics for the current process.
    pub fn collect() -> Self {
        platform::collect()
    }

    /// Format `memory_bytes` as a human-readable string (B / KB / MB / GB).
    pub fn format_memory(&self) -> String {
        match self.memory_bytes {
            None => "N/A".to_string(),
            Some(bytes) => format_bytes(bytes),
        }
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

// ── Linux ────────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
mod platform {
    use super::ProcessStats;
    use std::fs;

    pub fn collect() -> ProcessStats {
        let status = fs::read_to_string("/proc/self/status").ok();
        ProcessStats {
            memory_bytes: status.as_deref().and_then(parse_vm_rss),
            thread_count: status.as_deref().and_then(parse_threads),
        }
    }

    fn parse_vm_rss(status: &str) -> Option<u64> {
        for line in status.lines() {
            if let Some(rest) = line.strip_prefix("VmRSS:") {
                // Value is in kB, convert to bytes.
                let kb: u64 = rest.trim().strip_suffix("kB")?.trim().parse().ok()?;
                return Some(kb * 1024);
            }
        }
        None
    }

    fn parse_threads(status: &str) -> Option<u32> {
        for line in status.lines() {
            if let Some(rest) = line.strip_prefix("Threads:") {
                return rest.trim().parse().ok();
            }
        }
        None
    }
}

// ── Windows ──────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
mod platform {
    use super::ProcessStats;

    // Win32 FFI declarations (subset needed for memory + thread queries).
    #[allow(non_snake_case)]
    #[repr(C)]
    struct PROCESS_MEMORY_COUNTERS {
        cb: u32,
        PageFaultCount: u32,
        PeakWorkingSetSize: usize,
        WorkingSetSize: usize,
        QuotaPeakPagedPoolUsage: usize,
        QuotaPagedPoolUsage: usize,
        QuotaPeakNonPagedPoolUsage: usize,
        QuotaNonPagedPoolUsage: usize,
        PagefileUsage: usize,
        PeakPagefileUsage: usize,
    }

    #[allow(non_snake_case)]
    #[repr(C)]
    struct THREADENTRY32 {
        dwSize: u32,
        cntUsage: u32,
        th32ThreadID: u32,
        th32OwnerProcessID: u32,
        tpBasePri: i32,
        tpDeltaPri: i32,
        dwFlags: u32,
    }

    #[allow(clippy::upper_case_acronyms)]
    type HANDLE = *mut core::ffi::c_void;
    const INVALID_HANDLE_VALUE: HANDLE = -1_isize as HANDLE;
    const TH32CS_SNAPTHREAD: u32 = 0x00000004;

    unsafe extern "system" {
        // kernel32
        fn GetCurrentProcess() -> HANDLE;
        fn GetCurrentProcessId() -> u32;
        fn CreateToolhelp32Snapshot(dwFlags: u32, th32ProcessID: u32) -> HANDLE;
        fn Thread32First(hSnapshot: HANDLE, lpte: *mut THREADENTRY32) -> i32;
        fn Thread32Next(hSnapshot: HANDLE, lpte: *mut THREADENTRY32) -> i32;
        fn CloseHandle(hObject: HANDLE) -> i32;

        // psapi (kernel32 on modern Windows)
        #[link_name = "K32GetProcessMemoryInfo"]
        fn GetProcessMemoryInfo(
            hProcess: HANDLE,
            ppsmemCounters: *mut PROCESS_MEMORY_COUNTERS,
            cb: u32,
        ) -> i32;
    }

    pub fn collect() -> ProcessStats {
        ProcessStats {
            memory_bytes: get_memory(),
            thread_count: get_thread_count(),
        }
    }

    fn get_memory() -> Option<u64> {
        unsafe {
            let mut pmc = core::mem::zeroed::<PROCESS_MEMORY_COUNTERS>();
            pmc.cb = core::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
            if GetProcessMemoryInfo(GetCurrentProcess(), &mut pmc, pmc.cb) != 0 {
                Some(pmc.WorkingSetSize as u64)
            } else {
                None
            }
        }
    }

    fn get_thread_count() -> Option<u32> {
        unsafe {
            let pid = GetCurrentProcessId();
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
            if snap == INVALID_HANDLE_VALUE {
                return None;
            }

            let mut entry = core::mem::zeroed::<THREADENTRY32>();
            entry.dwSize = core::mem::size_of::<THREADENTRY32>() as u32;

            let mut count: u32 = 0;
            if Thread32First(snap, &mut entry) != 0 {
                loop {
                    if entry.th32OwnerProcessID == pid {
                        count += 1;
                    }
                    if Thread32Next(snap, &mut entry) == 0 {
                        break;
                    }
                }
            }

            CloseHandle(snap);
            Some(count)
        }
    }
}

// ── macOS ────────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod platform {
    use super::ProcessStats;

    // mach types
    type MachPort = u32;
    type KernReturn = i32;

    const MACH_TASK_BASIC_INFO: u32 = 20;
    const KERN_SUCCESS: i32 = 0;

    #[allow(non_snake_case)]
    #[repr(C)]
    struct MachTaskBasicInfo {
        virtual_size: u64,
        resident_size: u64,
        resident_size_max: u64,
        user_time: [u32; 2], // time_value_t (seconds, microseconds)
        system_time: [u32; 2],
        policy: i32,
        suspend_count: i32,
    }

    // libproc
    const PROC_PIDTASKINFO: i32 = 4;

    #[allow(non_snake_case)]
    #[repr(C)]
    struct ProcTaskInfo {
        pti_virtual_size: u64,
        pti_resident_size: u64,
        pti_total_user: u64,
        pti_total_system: u64,
        pti_threads_user: u64,
        pti_threads_system: u64,
        pti_policy: i32,
        pti_faults: i32,
        pti_pageins: i32,
        pti_cow_faults: i32,
        pti_messages_sent: i32,
        pti_messages_received: i32,
        pti_syscalls_mach: i32,
        pti_syscalls_unix: i32,
        pti_csw: i32,
        pti_threadnum: i32,
        pti_numrunning: i32,
        pti_priority: i32,
    }

    unsafe extern "C" {
        fn mach_task_self() -> MachPort;
        fn task_info(
            target_task: MachPort,
            flavor: u32,
            task_info_out: *mut MachTaskBasicInfo,
            task_info_count: *mut u32,
        ) -> KernReturn;
        fn proc_pidinfo(
            pid: i32,
            flavor: i32,
            arg: u64,
            buffer: *mut ProcTaskInfo,
            buffersize: i32,
        ) -> i32;
    }

    pub fn collect() -> ProcessStats {
        ProcessStats {
            memory_bytes: get_memory(),
            thread_count: get_thread_count(),
        }
    }

    fn get_memory() -> Option<u64> {
        unsafe {
            let mut info = core::mem::zeroed::<MachTaskBasicInfo>();
            let mut count =
                (core::mem::size_of::<MachTaskBasicInfo>() / core::mem::size_of::<u32>()) as u32;
            if task_info(
                mach_task_self(),
                MACH_TASK_BASIC_INFO,
                &mut info,
                &mut count,
            ) == KERN_SUCCESS
            {
                Some(info.resident_size)
            } else {
                None
            }
        }
    }

    fn get_thread_count() -> Option<u32> {
        unsafe {
            let pid = std::process::id() as i32;
            let mut info = core::mem::zeroed::<ProcTaskInfo>();
            let size = core::mem::size_of::<ProcTaskInfo>() as i32;
            let ret = proc_pidinfo(pid, PROC_PIDTASKINFO, 0, &mut info, size);
            if ret > 0 {
                Some(info.pti_threadnum as u32)
            } else {
                None
            }
        }
    }
}

// ── Unsupported platforms ────────────────────────────────────────────────────

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
mod platform {
    use super::ProcessStats;

    pub fn collect() -> ProcessStats {
        ProcessStats {
            memory_bytes: None,
            thread_count: None,
        }
    }
}

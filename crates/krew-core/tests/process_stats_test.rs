use krew_core::process_stats::ProcessStats;

#[test]
fn format_memory_human_readable() {
    let cases = [
        (Some(0), "0 B"),
        (Some(512), "512 B"),
        (Some(1024), "1.00 KB"),
        (Some(1536), "1.50 KB"),
        (Some(15_728_640), "15.00 MB"),
        (Some(1_073_741_824), "1.00 GB"),
        (None, "N/A"),
    ];
    for (bytes, expected) in cases {
        let stats = ProcessStats {
            memory_bytes: bytes,
            thread_count: None,
        };
        assert_eq!(stats.format_memory(), expected, "bytes={bytes:?}");
    }
}

#[test]
fn collect_returns_values_on_supported_platform() {
    let stats = ProcessStats::collect();
    #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
    {
        assert!(stats.memory_bytes.is_some(), "memory should be available");
        assert!(
            stats.thread_count.is_some(),
            "thread count should be available"
        );
    }
}

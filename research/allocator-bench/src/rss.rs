//! Process memory statistics from `/proc/self/status`.

fn field_kb(name: &str) -> usize {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix(name) {
            let kb: String = rest.chars().filter(|c| c.is_ascii_digit()).collect();
            return kb.parse().unwrap_or(0);
        }
    }
    0
}

/// Resident set size in bytes.
pub fn rss() -> usize {
    field_kb("VmRSS:") * 1024
}

/// Anonymous memory backed by transparent huge pages, in bytes (from smaps_rollup).
pub fn anon_huge() -> usize {
    let s = std::fs::read_to_string("/proc/self/smaps_rollup").unwrap_or_default();
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("AnonHugePages:") {
            let kb: String = rest.chars().filter(|c| c.is_ascii_digit()).collect();
            return kb.parse::<usize>().unwrap_or(0) * 1024;
        }
    }
    0
}

pub fn mib(bytes: usize) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

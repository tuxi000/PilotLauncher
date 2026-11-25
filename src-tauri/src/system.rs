//! 系统信息：物理内存与自动内存分配。

#[cfg(windows)]
pub fn total_memory_mb() -> u64 {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    // SAFETY: MEMORYSTATUSEX 是纯数据结构，按文档先设置 dwLength 再调用
    unsafe {
        let mut status: MEMORYSTATUSEX = std::mem::zeroed();
        status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        if GlobalMemoryStatusEx(&mut status) != 0 {
            return status.ullTotalPhys / 1024 / 1024;
        }
    }
    8192
}

#[cfg(not(windows))]
pub fn total_memory_mb() -> u64 {
    8192
}

#[cfg(windows)]
pub fn available_memory_mb() -> u64 {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    // SAFETY: 同上
    unsafe {
        let mut status: MEMORYSTATUSEX = std::mem::zeroed();
        status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        if GlobalMemoryStatusEx(&mut status) != 0 {
            return status.ullAvailPhys / 1024 / 1024;
        }
    }
    4096
}

#[cfg(not(windows))]
pub fn available_memory_mb() -> u64 {
    4096
}

/// 自动内存：按物理内存分档，原版游戏 2~6 GB 足够
pub fn auto_memory_mb() -> u32 {
    let total = total_memory_mb();
    let tier = if total <= 4096 {
        1536
    } else if total <= 8192 {
        3072
    } else if total <= 16384 {
        4096
    } else {
        6144
    };
    // 与 PCL 类似：同时参考当前可用内存，至少给系统留 512 MB，避免分配过多导致卡顿
    let cap = available_memory_mb().saturating_sub(512).max(1024);
    let mb = tier.min(cap) / 256 * 256;
    mb.max(1024) as u32
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryStatus {
    pub total_mb: u64,
    pub available_mb: u64,
    pub auto_mb: u32,
}

pub fn memory_status() -> MemoryStatus {
    MemoryStatus {
        total_mb: total_memory_mb(),
        available_mb: available_memory_mb(),
        auto_mb: auto_memory_mb(),
    }
}

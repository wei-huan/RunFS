// 对FSInfo的抽象

use super::{BlockDevice, START_CLUS_ID};
use crate::error::FSError;
#[cfg(not(feature = "std"))]
use alloc::slice;
use alloc::sync::Arc;

const NO_INFORMATION: u32 = 0xFFFFFFFF;

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct FSInfo {
    free_cluster_count: Option<u32>,
    next_free_cluster: Option<u32>,
}

impl FSInfo {
    pub fn new(free_cluster_count: u32, next_free_cluster: u32) -> Self {
        Self {
            free_cluster_count: if free_cluster_count != NO_INFORMATION {
                Some(free_cluster_count)
            } else {
                None
            },
            next_free_cluster: if free_cluster_count != NO_INFORMATION {
                Some(next_free_cluster)
            } else {
                None
            },
        }
    }
    pub fn validate_and_fix(&mut self, total_clusters: u32) {
        let max_valid_cluster_number = total_clusters + (START_CLUS_ID as u32);
        if let Some(n) = self.free_cluster_count {
            if n > total_clusters {
                self.free_cluster_count = None;
            }
        }
        if let Some(n) = self.next_free_cluster {
            // values 0 and 1 are reserved
            if n > max_valid_cluster_number || n == 0 | 1 {
                self.next_free_cluster = None;
            }
        }
    }
    pub fn map_free_clusters(&mut self, map_fn: impl Fn(u32) -> u32) {
        if let Some(n) = self.free_cluster_count {
            self.free_cluster_count = Some(map_fn(n));
        }
    }
    /// 返回 None 只是代表不确定而已
    pub fn next_free_cluster(&self) -> Option<u32> {
        self.next_free_cluster
    }
    /// 返回 None 只是代表不确定而已
    pub fn free_clusters(&self) -> Option<u32> {
        self.free_cluster_count
    }
    pub fn set_next_free_cluster(&mut self, cluster_id: Option<u32>) {
        self.next_free_cluster = cluster_id;
    }
    pub fn set_free_cluster_count(&mut self, free_cluster_count: Option<u32>) {
        self.free_cluster_count = free_cluster_count;
    }
}

#[repr(C, packed(1))]
#[derive(Debug, Copy, Clone)]
pub(crate) struct FSInfoSector {
    lead_signature: u32,
    dummy1: [u8; 480],
    struc_signature: u32,
    free_cluster_count: u32,
    next_free_cluster: u32,
    dummy2: [u8; 12],
    trail_signature: u32,
}

impl Default for FSInfoSector {
    fn default() -> FSInfoSector {
        FSInfoSector {
            lead_signature: 0,
            dummy1: [0; 480],
            struc_signature: 0,
            free_cluster_count: 0,
            next_free_cluster: 0,
            dummy2: [0; 12],
            trail_signature: 0,
        }
    }
}

impl FSInfoSector {
    const LEAD_SIGNATURE: u32 = 0x4161_5252;
    const STRUC_SIGNATURE: u32 = 0x6141_7272;
    const TRAIL_SIGNATURE: u32 = 0xAA55_0000;

    // 直接通过块设备读取获得启动扇区, 只用于 RunFileSystem 创建
    pub(crate) fn directly_new(fsinfo_block_id: usize, block_device: Arc<dyn BlockDevice>) -> Self {
        // println!("size of BootSector: {}", core::mem::size_of::<BootSector>());
        let fsinfo_sector = FSInfoSector::default();
        // 调试没问题,能够获取 512 Byte 准确数据
        let sector_slice = unsafe {
            slice::from_raw_parts_mut(
                (&fsinfo_sector as *const FSInfoSector) as *mut u8,
                core::mem::size_of::<FSInfoSector>(),
            )
        };
        block_device
            .read_block(fsinfo_block_id, sector_slice)
            .unwrap();
        fsinfo_sector
    }
    pub(crate) fn from_fsinfo(fsinfo: FSInfo) -> Self {
        Self {
            lead_signature: Self::LEAD_SIGNATURE,
            struc_signature: Self::STRUC_SIGNATURE,
            trail_signature: Self::TRAIL_SIGNATURE,
            free_cluster_count: if let Some(count) = fsinfo.free_cluster_count {
                count
            } else {
                NO_INFORMATION
            },
            next_free_cluster: if let Some(next) = fsinfo.next_free_cluster {
                next
            } else {
                NO_INFORMATION
            },
            ..Self::default()
        }
    }
    #[must_use]
    pub(crate) fn validate(&self) -> Result<(), FSError> {
        if self.lead_signature != Self::LEAD_SIGNATURE
            || self.struc_signature != Self::STRUC_SIGNATURE
            || self.trail_signature != Self::TRAIL_SIGNATURE
        {
            // println!("invalid signature in FSInfo");
            return Err(FSError::CorruptedFileSystem);
        }
        Ok(())
    }
    /// 返回 None 只是代表不确定而已
    pub fn next_free_cluster_raw(&self) -> u32 {
        self.next_free_cluster
    }
    /// 返回 None 只是代表不确定而已
    pub fn free_clusters_raw(&self) -> u32 {
        self.free_cluster_count
    }
}

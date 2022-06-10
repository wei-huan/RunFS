// 对FSInfo的抽象

use super::BlockDevice;
use crate::error::FSError;
use std::slice;
use std::sync::Arc;

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct FSInfo {
    free_cluster_count: u32,
    next_free_cluster: u32,
}

impl FSInfo {
    #[must_use]
    pub fn free_cluster(&self) -> u32 {
        self.next_free_cluster
    }
    #[must_use]
    pub fn free_cluster_count(&self) -> u32 {
        self.free_cluster_count
    }
    #[must_use]
    pub fn set_next_free_cluster(&mut self, cluster: u32) {
        self.next_free_cluster = cluster;
    }
    #[must_use]
    pub fn set_free_cluster_count(&mut self, free_cluster_count: u32) {
        self.free_cluster_count = free_cluster_count;
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub(crate) struct FSInfoSector {
    lead_signature: u32,
    dummy1: [u8; 480],
    struc_signature: u32,
    pub(crate) fsinfo: FSInfo,
    dummy2: [u8; 12],
    trail_signature: u32,
}

impl Default for FSInfoSector {
    fn default() -> FSInfoSector {
        FSInfoSector {
            lead_signature: 0,
            dummy1: [0; 480],
            struc_signature: 0,
            fsinfo: FSInfo::default(),
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
        block_device.read_block(fsinfo_block_id, sector_slice).unwrap();
        fsinfo_sector
    }

    // pub(crate) fn new(block_device: Arc<dyn BlockDevice>) -> Self {
    //     let fsinfo_sector: FSInfoSector = get_info_cache(1, Arc::clone(&block_device))
    //         .read()
    //         .read(0, |fs: &FSInfoSector| *fs);
    //     fsinfo_sector
    // }

    #[must_use]
    pub(crate) fn validate(&self) -> Result<(), FSError> {
        if self.lead_signature != Self::LEAD_SIGNATURE
            || self.struc_signature != Self::STRUC_SIGNATURE
            || self.trail_signature != Self::TRAIL_SIGNATURE
        {
            println!("invalid signature in FSInfo");
            return Err(FSError::CorruptedFileSystem);
        }
        Ok(())
    }
}

// 对FSInfo的抽象

use super::BlockDevice;
use crate::error::FSError;
use crate::sector_cache::get_info_cache;
use std::sync::Arc;

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct FSInfo {
    free_cluster_count: u32,
    next_free_cluster: u32,
}

impl FSInfo {
    #[allow(unused)]
    fn new(block_device: Arc<dyn BlockDevice>) -> Self {
        let fsinfo: FSInfo = get_info_cache(1, Arc::clone(&block_device))
            .read()
            .read(488, |fsinfo: &FSInfo| *fsinfo);
        fsinfo
    }
    #[must_use]
    fn free_cluster(&self) -> u32 {
        self.next_free_cluster
    }
    #[must_use]
    fn cluster_count(&self) -> u32 {
        self.free_cluster_count
    }
    #[must_use]
    fn set_next_free_cluster(&mut self, cluster: u32) {
        self.next_free_cluster = cluster;
    }
    #[must_use]
    fn set_free_cluster_count(&mut self, free_cluster_count: u32) {
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

    #[must_use]
    pub(crate) fn new(block_device: Arc<dyn BlockDevice>) -> Self {
        let fsinfo_sector: FSInfoSector = get_info_cache(1, Arc::clone(&block_device))
            .read()
            .read(0, |fs: &FSInfoSector| *fs);
        fsinfo_sector
    }

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

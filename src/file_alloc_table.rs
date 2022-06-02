// FAT 表结构体
use super::{get_info_cache, BlockDevice, FAT_TABLE_START_SEC, SEC_SZ, SectorCache};
use std::sync::Arc;

const END_CLU: u32 = 0x0FFFFFFF;
const BAD_CLU: u32 = 0xFFFFFFF7;
const UNUSED_CLU: u32 = 0;
const FAT_ENTRY_BYTES: usize = 4;
const FAT_ENTRY_WIDTH: usize = 32;
const ENTRYS_PER_SEC: usize = SEC_SZ / FAT_ENTRY_BYTES;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct FATEntry(pub u32);

impl FATEntry {
    pub fn is_available_clu(&self) -> bool {
        self.0 == UNUSED_CLU
    }
    pub fn is_end_clu(&self) -> bool {
        self.0 == END_CLU
    }
    pub fn is_broken_clu(&self) -> bool {
        self.0 == BAD_CLU
    }
    // pub fn read_entry(&self, cluster_id: usize, block_device: Arc<dyn BlockDevice>) -> &FATEntry {
    //     assert!(cluster_id >= 2);
    //     let sector_id = (cluster_id - 2) / ENTRYS_PER_SEC + FAT_TABLE_START_SEC;
    //     let offset = ((cluster_id - 2) % ENTRYS_PER_SEC) * FAT_ENTRY_BYTES;
    //     get_sector_cache(sector_id, Arc::clone(&block_device))
    //         .lock()
    //         .read(0, |fat_entry_sector: &SectorCache| fat_entry_sector[offset])
    // }
    // pub fn set_entry(&self, cluster_id: usize, entry_data: u32, block_device: Arc<dyn BlockDevice>) -> &FATEntry {
    //     assert!(cluster_id >= 2);
    //     let sector_id = (cluster_id - 2) / ENTRYS_PER_SEC + FAT_TABLE_START_SEC;
    //     let offset = ((cluster_id - 2) % ENTRYS_PER_SEC) * FAT_ENTRY_BYTES;
    //     get_sector_cache(sector_id, Arc::clone(&block_device))
    //         .lock()
    //         .read(0, |fat_entry_sector: &SectorCache| fat_entry_sector[offset])
    // }
}

// pub struct FATRegion {}

// impl FATRegion {
// }

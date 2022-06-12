// FAT 表结构体
use super::{BiosParameterBlock, BlockDevice, FSInfo, SectorCacheManager};
use std::sync::Arc;

const BYTES_PER_ENTRY: usize = 4;

/// The  high  4  bits  of  a  FAT32  FAT  entry  are  reserved.
/// No FAT32 volume should ever be configured containing cluster numbers available for
/// allocation >= 0xFFFFFF7.
/// There is no limit on the size of the FAT on volumes formatted FAT32.
#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum FATEntry {
    Bad,
    Free,
    Final,
    Next(u32),
}

// pub struct FileAllocationTable {
//     fat1_sector: usize,
//     fat2_sector: usize,
//     entrys_per_sec: usize,
// }

// impl FileAllocationTable {
//     pub fn new(fat1_sector: usize, fat2_sector: usize, entrys_per_sec: usize) -> Self {
//         Self {
//             fat1_sector,
//             fat2_sector,
//             entrys_per_sec,
//         }
//     }
//     /// 前为 FAT1 的扇区号，后为 FAT2 的扇区号，最后为offset
//     fn position(&self, cluster_id: usize) -> (usize, usize, usize) {
//         let fat1_sec = self.fat1_sector + cluster_id / self.entrys_per_sec;
//         let fat2_sec = self.fat2_sector + cluster_id / self.entrys_per_sec;
//         let offset = BYTES_PER_ENTRY * (cluster_id % self.entrys_per_sec);
//         (fat1_sec, fat2_sec, offset)
//     }
// }


/// 管理 FAT 和 FSINFO
pub struct FATManager {
    pub fsinfo: Arc<FSInfo>,
    pub sector_cache: SectorCacheManager,
}

impl FATManager {
    pub fn new(
        fsinfo: Arc<FSInfo>,
        bpb: Arc<BiosParameterBlock>,
        block_device: Arc<dyn BlockDevice>,
    ) -> Self {
        Self {
            fsinfo,
            sector_cache: SectorCacheManager::new(bpb, block_device),
        }
    }
}

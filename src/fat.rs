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

/// 管理 FAT 和 FSINFO
pub struct FATManager {
    fsinfo: FSInfo,
    pub bpb: Arc<BiosParameterBlock>,
    pub sector_cache: SectorCacheManager,
}

impl FATManager {
    pub fn new(
        fsinfo: FSInfo,
        bpb: Arc<BiosParameterBlock>,
        block_device: Arc<dyn BlockDevice>,
    ) -> Self {
        Self {
            bpb: Arc::clone(&bpb),
            fsinfo,
            sector_cache: SectorCacheManager::new(bpb, block_device),
        }
    }
    pub fn fsinfo(&self) -> FSInfo {
        self.fsinfo
    }
    pub fn entrys_per_sector(&self) -> usize {
        self.bpb.bytes_per_sector() as usize / BYTES_PER_ENTRY
    }
    pub fn position(&self, cluster_id: usize) -> (usize, usize, usize) {
        let fat_sector: usize =
            self.bpb.first_fats_sector() as usize + (cluster_id / self.entrys_per_sector());
        let backup_fat_sector: usize =
            self.bpb.first_backup_fats_sector() as usize + (cluster_id / self.entrys_per_sector());
        let offset = BYTES_PER_ENTRY * (cluster_id % self.entrys_per_sector());
        (fat_sector, backup_fat_sector, offset)
    }
    pub fn fat_entry(&mut self, cluster_id: usize) -> FATEntry {
        let (sector_id, _, offset) = self.position(cluster_id);
        println!("sector_id: {}, offset: {}", sector_id, offset);
        let sector = self.sector_cache.get_cache(sector_id);
        println!("size of FATEntry {}", core::size_of<FATEntry>());
        let entry = sector.read().read(offset, |e: &FATEntry| *e);
        entry
    }
}

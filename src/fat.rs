// FAT 表结构体
use super::{BiosParameterBlock, BlockDevice, FSInfo, SectorCacheManager, START_CLUS_ID};
use std::sync::Arc;

const BYTES_PER_ENTRY: usize = 4;
const BAD_CLUSTER: u32 = 0x0FFF_FFF7;
const FINAL_CLUSTER: u32 = 0x0FFF_FFFF;

/// The high 4 bits of a FAT32 FAT entry are reserved.
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
    fn entry_raw(&mut self, cluster_id: usize) -> u32 {
        let (sector_id, _, offset) = self.position(cluster_id);
        let sector = self.sector_cache.get_cache(sector_id);
        let entry_raw = sector.read().read(offset, |e: &u32| *e);
        entry_raw
    }
    pub fn entry(&mut self, cluster_id: usize) -> FATEntry {
        assert!(
            cluster_id <= self.bpb.total_clusters() as usize + START_CLUS_ID,
            "Invalid Cluster ID in FAT"
        );
        let entry_raw = self.entry_raw(cluster_id) & 0x0FFF_FFFF;
        match entry_raw {
            0 if (0x0FFF_FFF7..=0x0FFF_FFFF).contains(&cluster_id) => FATEntry::Bad, // avoid accidental use or allocation into a FAT chain
            0 => FATEntry::Free,
            0x0FFF_FFF7 => FATEntry::Bad,
            0x0FFF_FFF8..=0x0FFF_FFFF => FATEntry::Final,
            _n if (0x0FFF_FFF7..=0x0FFF_FFFF).contains(&cluster_id) => FATEntry::Bad, // avoid accidental use or allocation into a FAT chain
            n => FATEntry::Next(n),
        }
    }
    fn set_entry_raw(&mut self, cluster_id: usize, value: u32) {
        let (sector_id, _, offset) = self.position(cluster_id);
        let sector = self.sector_cache.get_cache(sector_id);
        sector.write().modify(offset, |e: &mut u32| *e = value);
    }
    pub fn set_entry(&mut self, cluster_id: usize, entry: FATEntry) {
        assert!(
            cluster_id <= self.bpb.total_clusters() as usize + START_CLUS_ID,
            "Invalid Cluster ID in FAT"
        );
        let old_reserved_bits = self.entry_raw(cluster_id) & 0xF000_0000;
        if entry == FATEntry::Free && cluster_id >= 0x0FFF_FFF7 && cluster_id <= 0x0FFF_FFFF {
            let tmp = if cluster_id == 0x0FFF_FFF7 {
                "BAD_CLUSTER"
            } else {
                "end-of-chain"
            };
            panic!(
                "cluster number {} is a special value in FAT to indicate {}; it should never be set as free",
                cluster_id, tmp
            );
        };
        let value = match entry {
            FATEntry::Free => 0,
            FATEntry::Bad => BAD_CLUSTER,
            FATEntry::Final => FINAL_CLUSTER,
            FATEntry::Next(n) => n,
        };
        let value = value | old_reserved_bits; // must preserve original reserved values
        self.set_entry_raw(cluster_id, value);
    }
}

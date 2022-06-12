// FAT 表结构体
use super::{BiosParameterBlock, BlockDevice, FSInfo, SectorCacheManager, START_CLUS_ID};
use std::sync::Arc;

const BYTES_PER_ENTRY: usize = 4;
const BAD_CLUSTER: u32 = 0x0FFF_FFF7;
const FINAL_CLUSTER: u32 = 0x0FFF_FFFF;

/// The high 4 bits of a FAT32 FAT entry are reserved.
/// No FAT32 volume should ever be configured containing cluster numbers available for
/// allocation >= 0xFFFFFF7.
#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum FATEntry {
    Bad,
    Free,
    End,
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
            0x0FFF_FFF8..=0x0FFF_FFFF => FATEntry::End,
            _n if (0x0FFF_FFF7..=0x0FFF_FFFF).contains(&cluster_id) => FATEntry::Bad, // avoid accidental use or allocation into a FAT chain
            n => FATEntry::Next(n),
        }
    }
    fn set_entry_raw(&mut self, cluster_id: usize, value: u32) {
        let (sector_id, backup_sector_id, offset) = self.position(cluster_id);
        // FAT1
        let sector = self.sector_cache.get_cache(sector_id);
        sector.write().modify(offset, |e: &mut u32| *e = value);
        // FAT2
        let backup_sector = self.sector_cache.get_cache(backup_sector_id);
        backup_sector
            .write()
            .modify(offset, |e: &mut u32| *e = value);
    }
    pub fn set_entry(&mut self, cluster_id: usize, entry: FATEntry) {
        assert!(
            ((cluster_id <= self.bpb.total_clusters() as usize + START_CLUS_ID)
                && (cluster_id >= START_CLUS_ID)),
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
            FATEntry::End => FINAL_CLUSTER,
            FATEntry::Next(n) => n,
        };
        let value = value | old_reserved_bits; // must preserve original reserved values
        self.set_entry_raw(cluster_id, value);
    }
    pub fn next_cluster(&mut self, cluster_id: usize) -> Option<usize> {
        let val = self.entry(cluster_id);
        match val {
            FATEntry::Next(n) => Some(n as usize),
            _ => None,
        }
    }
    pub fn set_next_cluster(&mut self, cluster_id: usize, next_cluster: u32) {
        self.set_entry(cluster_id, FATEntry::Next(next_cluster));
    }
    pub fn set_end(&mut self, cluster_id: usize) {
        self.set_entry(cluster_id, FATEntry::End);
    }
    pub fn set_free(&mut self, cluster_id: usize) {
        self.set_entry(cluster_id, FATEntry::Free);
    }
    pub fn set_bad(&mut self, cluster_id: usize) {
        self.set_entry(cluster_id, FATEntry::Bad);
    }
    pub fn final_cluster(&mut self, start_cluster: usize) -> usize {
        let mut curr_cluster = start_cluster;
        // assert_ne!(start_cluster, 0);
        loop {
            if let Some(next_cluster) = self.next_cluster(curr_cluster) {
                curr_cluster = next_cluster;
            } else {
                return curr_cluster & 0x0FFFFFFF;
            }
        }
    }
    pub fn all_clusters(&mut self, start_cluster: usize) -> Vec<usize> {
        let mut curr_cluster = start_cluster;
        let mut clusters: Vec<usize> = Vec::new();
        loop {
            clusters.push(curr_cluster & 0x0FFFFFFF);
            if let Some(next_cluster) = self.next_cluster(curr_cluster) {
                curr_cluster = next_cluster;
            } else {
                return clusters;
            }
        }
    }
    pub fn count_clasters(&mut self, start_cluster: usize) -> usize {
        let mut curr_cluster = start_cluster;
        let mut num = 0;
        loop {
            num += 1;
            if let Some(next_cluster) = self.next_cluster(curr_cluster) {
                curr_cluster = next_cluster;
            } else {
                return num;
            }
        }
    }
    // pub fn find_free(&self) -> Option<usize> {
    //     None
    // }
    // pub fn count_free(&self) -> usize {}
}

// FAT 表结构体
use super::{
    BiosParameterBlock, BlockDevice, FSInfo, FSInfoSector, SectorCacheManager, START_CLUS_ID,
};
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
    bpb: Arc<BiosParameterBlock>,
    sector_cache: SectorCacheManager,
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
    /// 返回 None 只是代表不确定而已
    pub fn next_free_cluster(&self) -> Option<u32> {
        self.fsinfo.next_free_cluster()
    }
    /// 返回 None 只是代表不确定而已
    pub fn free_clusters(&self) -> Option<u32> {
        self.fsinfo.free_clusters()
    }
    pub fn fsinfo(&self) -> FSInfo {
        self.fsinfo
    }
    fn entrys_per_sector(&self) -> usize {
        self.bpb.bytes_per_sector() as usize / BYTES_PER_ENTRY
    }
    fn position(&self, cluster_id: usize) -> (usize, usize, usize) {
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
                "End-of-Chain"
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
    pub fn count_clusters(&mut self, start_cluster: usize) -> usize {
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
    /// 在 FSINFO 没有提供的情况下使用, 返回 None 代表没有空闲簇了
    /// 如果 FSINFO 中有空簇且 id > start_cluster 就返回空簇, 否则没有就从起始簇开始线性搜索
    pub fn search_free_cluster(&mut self, start_cluster: usize) -> Option<u32> {
        let end_cluster = self.bpb.total_clusters() as usize + START_CLUS_ID;
        assert!(
            start_cluster < end_cluster,
            "Invalid start cluster in searching"
        );
        let next = self.fsinfo.next_free_cluster();
        if next.is_some() && next.unwrap() > (start_cluster as u32) {
            return next;
        } else {
            // 从当前搜到末尾
            let mut cluster_id = start_cluster;
            while cluster_id < end_cluster {
                if self.entry(cluster_id) == FATEntry::Free {
                    return Some(cluster_id as u32);
                }
                cluster_id += 1;
            }
            // 从开始搜到当前
            let end_cluster = start_cluster;
            cluster_id = START_CLUS_ID;
            while cluster_id < end_cluster {
                if self.entry(cluster_id) == FATEntry::Free {
                    return Some(cluster_id as u32);
                }
                cluster_id += 1;
            }
            return None;
        }
    }
    /// 在 FSINFO 没有提供的情况下使用, 返回 0 代表没有空闲簇了
    pub fn count_free_clusters(&mut self) -> u32 {
        if let Some(num) = self.fsinfo.free_clusters() {
            return num;
        } else {
            // 从开始搜到末尾
            let mut cluster_id = START_CLUS_ID;
            let mut num = 0;
            let end_cluster = self.bpb.total_clusters() as usize + START_CLUS_ID;
            while cluster_id < end_cluster {
                if self.entry(cluster_id) == FATEntry::Free {
                    num += 1;
                }
                cluster_id += 1;
            }
            return num;
        }
    }
    /// 只是返回可以使用的簇 ID, 不对簇清零或者提供别的功能
    pub fn alloc_cluster(&mut self, prev: Option<u32>) -> Option<u32> {
        if let Some(free_id) = self.search_free_cluster(START_CLUS_ID) {
            self.set_entry(free_id as usize, FATEntry::End);
            if let Some(prev) = prev {
                self.set_entry(prev as usize, FATEntry::Next(free_id));
            }
            let next_free = self.search_free_cluster(free_id as usize);
            self.fsinfo.set_next_free_cluster(next_free);
            self.fsinfo.map_free_clusters(|n| n - 1);
            return Some(free_id);
        } else {
            return None;
        }
    }
    /// 只是返回可以使用的第一个簇 ID, 如果需要的簇不够, 就直接返回 None
    pub fn alloc_clusters(&mut self, num: usize, prev: Option<u32>) -> Option<u32> {
        let free_clusters = self.fsinfo.free_clusters();
        if free_clusters.is_some() && num <= free_clusters.unwrap() as usize {
            let mut prev_id = prev;
            let mut first: Option<u32> = None;
            for i in 0..num {
                prev_id = self.alloc_cluster(prev_id);
                if i == 0 {
                    first = prev_id;
                }
            }
            return first;
        } else {
            return None;
        }
    }
    /// 返回簇链中第 n 个元素的 id
    pub fn search_cluster(&mut self, chain_start_cluster: usize, index: usize) -> Option<usize> {
        let mut curr_cluster = chain_start_cluster;
        for _ in 0..index {
            if let Some(next_cluster) = self.next_cluster(curr_cluster) {
                curr_cluster = next_cluster;
            } else {
                return None;
            }
        }
        return Some(curr_cluster);
    }
    // /// 从提供的 cluster_id 开始截断分配的簇链, 并把后面的簇都归还
    // pub fn truncate_cluster_chain(&mut self, cluster_id: u32) {
    //     self.fsinfo.map_free_clusters(|n| n + num_free);
    // }
    // /// 从提供的 cluster_id 开始归还分配的簇链
    // pub fn free_cluster_chain(&mut self, cluster_id: u32) {
    //     self.fsinfo.map_free_clusters(|n| n + num_free);
    // }
    /// 同步 FSINFO 回外存
    pub fn sync_fsinfo(&mut self) {
        let fsinfo_sector = FSInfoSector::from_fsinfo(self.fsinfo);
        let cache = self
            .sector_cache
            .get_cache(self.bpb.fsinfo_sector() as usize);
        cache
            .write()
            .modify(0, |s: &mut FSInfoSector| *s = fsinfo_sector);
    }
}

impl Drop for FATManager {
    fn drop(&mut self) {
        self.sync_fsinfo()
    }
}

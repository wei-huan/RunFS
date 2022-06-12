//对文件系统的全局管理.
use super::{
    BiosParameterBlock, BlockDevice, BootSector, ClusterCacheManager, FATManager, FSInfo,
    FSInfoSector,
};
use spin::RwLock;
use std::sync::Arc;

// 包括 BPB 和 FSInfo 的信息
pub struct RunFileSystem {
    pub bpb: BiosParameterBlock,
    pub fat_manager: Arc<RwLock<FATManager>>,
    pub block_device: Arc<dyn BlockDevice>,
    pub cluster_cache: ClusterCacheManager,
    // root_dir: Arc<RwLock<ShortDirectoryEntry>>, // 根目录项
}

impl RunFileSystem {
    pub fn new(block_device: Arc<dyn BlockDevice>) -> Self {
        let boot_sector = BootSector::directly_new(Arc::clone(&block_device));
        let res = boot_sector.validate();
        match res {
            Ok(v) => v,
            Err(e) => panic!("Bios Parameter Block not valid: {:?}", e),
        }
        let bpb = boot_sector.bpb;
        let fsinfo_block_id: usize = bpb.fsinfo_sector().try_into().unwrap();
        let fsinfo_sector = FSInfoSector::directly_new(fsinfo_block_id, Arc::clone(&block_device));
        let res = fsinfo_sector.validate();
        match res {
            Ok(v) => v,
            Err(e) => panic!("FSInfo Block not valid: {:?}", e),
        }
        let fsinfo = fsinfo_sector.fsinfo;
        Self {
            bpb,
            cluster_cache: ClusterCacheManager::new(Arc::new(bpb), Arc::clone(&block_device)),
            fat_manager: Arc::new(RwLock::new(FATManager::new(
                fsinfo,
                Arc::new(bpb),
                Arc::clone(&block_device),
            ))),
            block_device,
        }
    }
    /// Returns a volume identifier read from BPB in the Boot Sector.
    pub fn volume_id(&self) -> u32 {
        self.bpb.volumn_id()
    }
    pub fn bpb(&self) -> BiosParameterBlock {
        self.bpb
    }
    pub fn fsinfo(&self) -> FSInfo {
        self.fat_manager.read().fsinfo()
    }
}

//对文件系统的全局管理.
use super::{
    BiosParameterBlock, BlockDevice, BootSector, ClusterCacheManager, FATManager, FSInfoSector,
};
use spin::RwLock;
use std::sync::Arc;

// 包括 BPB 和 FSInfo 的信息
pub struct RunFileSystem {
    pub bpb: Arc<BiosParameterBlock>,
    pub fat_manager: Arc<RwLock<FATManager>>,
    pub block_device: Arc<dyn BlockDevice>,
    pub cluster_cache: ClusterCacheManager,
    // root_dir: Arc<RwLock<ShortDirectoryEntry>>, // 根目录项
}

impl RunFileSystem {
    #[must_use]
    pub fn new(block_device: Arc<dyn BlockDevice>) -> Self {
        // println!(
        //     "size of BiosParameterBlock: {}",
        //     core::mem::size_of::<BiosParameterBlock>()
        // );
        // println!("size of BootSector: {}", core::mem::size_of::<BootSector>());
        let boot_sector = BootSector::directly_new(Arc::clone(&block_device));
        // println!("BootSector: {:#X?}", boot_sector);
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
            bpb: Arc::new(bpb),
            cluster_cache: ClusterCacheManager::new(Arc::new(bpb), Arc::clone(&block_device)),
            fat_manager: Arc::new(RwLock::new(FATManager::new(
                Arc::new(fsinfo),
                Arc::new(bpb),
                Arc::clone(&block_device),
            ))),
            block_device,
        }
    }
}

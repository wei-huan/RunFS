//对文件系统的全局管理.
use super::{
    BiosParameterBlock, BlockDevice, BootSector, ClusterCacheManager, FATManager, FSInfo,
    FSInfoSector, FileAttributes, ShortDirectoryEntry, VFile,
};
use spin::RwLock;
use std::sync::Arc;

// 包括 BPB 和 FSInfo 的信息
pub struct RunFileSystem {
    pub bpb: BiosParameterBlock,
    pub fat_manager: Arc<RwLock<FATManager>>,
    pub block_device: Arc<dyn BlockDevice>,
    pub cluster_cache: ClusterCacheManager,
    root_dirent: Arc<RwLock<ShortDirectoryEntry>>, // 根目录项
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
        let mut fsinfo = FSInfo::new(
            fsinfo_sector.free_clusters_raw(),
            fsinfo_sector.next_free_cluster_raw(),
        );
        fsinfo.validate_and_fix(bpb.total_clusters());
        let mut root_dirent = ShortDirectoryEntry::new(
            [0x2F, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20], // .
            [0x20, 0x20, 0x20],
            FileAttributes::DIRECTORY,
        );
        root_dirent.set_first_cluster(Some(2));
        Self {
            bpb,
            cluster_cache: ClusterCacheManager::new(Arc::new(bpb), Arc::clone(&block_device)),
            fat_manager: Arc::new(RwLock::new(FATManager::new(
                fsinfo,
                Arc::new(bpb),
                Arc::clone(&block_device),
            ))),
            block_device,
            root_dirent: Arc::new(RwLock::new(root_dirent)),
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
    pub fn root_vfile(&self, fs_manager: &Arc<RwLock<Self>>) -> VFile {
        let long_pos_vec: Vec<(usize, usize)> = Vec::new();
        VFile::new(
            String::from("/"),
            0,
            0,
            long_pos_vec,
            FileAttributes::DIRECTORY,
            Arc::clone(fs_manager),
            self.block_device.clone(),
        )
    }
    pub fn root_dirent(&self) -> Arc<RwLock<ShortDirectoryEntry>> {
        self.root_dirent.clone()
    }
}

//对文件系统的全局管理.
use super::{
    BiosParameterBlock, BlockDevice, BootSector, DataManager, FATManager, FSInfo, FSInfoSector,
    FileAttributes, ShortDirectoryEntry, VFile,
};
use spin::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::sync::Arc;

// 包括 BPB 和 FSInfo 的信息
pub struct RunFileSystem {
    bpb: Arc<BiosParameterBlock>,
    fat_manager: Arc<RwLock<FATManager>>,
    data_manager: Arc<RwLock<DataManager>>,
    block_device: Arc<dyn BlockDevice>,
}

impl RunFileSystem {
    pub fn new(block_device: Arc<dyn BlockDevice>) -> Self {
        let boot_sector = BootSector::directly_new(Arc::clone(&block_device));
        let res = boot_sector.validate();
        match res {
            Ok(v) => v,
            Err(e) => panic!("Bios Parameter Block not valid: {:?}", e),
        }
        let bpb = Arc::new(boot_sector.bpb);
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
        root_dirent.set_first_cluster(bpb.root_dir_cluster());
        Self {
            bpb: bpb.clone(),
            fat_manager: Arc::new(RwLock::new(FATManager::new(
                fsinfo,
                bpb.clone(),
                Arc::clone(&block_device),
            ))),
            data_manager: Arc::new(RwLock::new(DataManager::new(
                bpb.clone(),
                Arc::new(RwLock::new(root_dirent)),
                Arc::clone(&block_device),
            ))),
            block_device,
        }
    }
    /// Returns a volume identifier read from BPB in the Boot Sector.
    pub fn volume_id(&self) -> u32 {
        self.bpb.volumn_id()
    }
    pub fn bpb(&self) -> Arc<BiosParameterBlock> {
        self.bpb.clone()
    }
    pub fn fat_manager_read(&self) -> RwLockReadGuard<FATManager> {
        self.fat_manager.read()
    }
    pub fn fat_manager_modify(&self) -> RwLockWriteGuard<FATManager> {
        self.fat_manager.write()
    }
    pub fn data_manager_read(&self) -> RwLockReadGuard<DataManager> {
        self.data_manager.read()
    }
    pub fn data_manager_modify(&self) -> RwLockWriteGuard<DataManager> {
        self.data_manager.write()
    }
    /// 返回 None 只是代表不确定而已
    pub fn next_free_cluster(&self) -> Option<u32> {
        self.fat_manager_read().next_free_cluster()
    }
    /// 返回 None 只是代表不确定而已
    pub fn free_clusters(&self) -> Option<u32> {
        self.fat_manager_read().free_clusters()
    }
    pub fn fsinfo(&self) -> FSInfo {
        self.fat_manager_read().fsinfo()
    }
    /// 在 FAT 表中分配项并清空对应簇中的数据, 成功返回 id, 失败返回 None
    pub fn alloc_cluster(&mut self) -> Option<u32> {
        if let Some(cluster_id) = self.fat_manager.write().alloc_cluster(None) {
            self.data_manager.write().clear_cluster(cluster_id as usize);
            return Some(cluster_id);
        }
        return None;
    }
    /// 在 FAT 表中分配多个项并清空对应簇中的数据, 成功返回分配的第一个 id, 失败返回 None
    pub fn alloc_clusters(&mut self, num: usize) -> Option<u32> {
        let mut fat_manager = self.fat_manager.write();
        if let Some(first_cluster) = fat_manager.alloc_clusters(num, None) {
            let id_vec = fat_manager.all_clusters(first_cluster as usize);
            for id in id_vec {
                self.data_manager.write().clear_cluster(id);
            }
            return Some(first_cluster);
        } else {
            return None;
        }
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
        )
    }
    // pub fn root_dirent(&self) -> Arc<RwLock<ShortDirectoryEntry>> {
    //     self.root_dirent.clone()
    // }
}

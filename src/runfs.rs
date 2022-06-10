//对文件系统的全局管理.
use super::BlockDevice;
use super::{BiosParameterBlock, BootSector, FSInfo, FSInfoSector};
use std::sync::Arc;

// 包括 BPB 和 FSInfo 的信息
pub struct RunFileSystem {
    pub(crate) bpb: BiosParameterBlock,
    pub(crate) fsinfo: FSInfo,
    block_device: Arc<dyn BlockDevice>,
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

        let fsinfo_sector = FSInfoSector::new(Arc::clone(&block_device));
        let res = fsinfo_sector.validate();
        match res {
            Ok(v) => v,
            Err(e) => panic!("FSInfo Block not valid: {:?}", e),
        }
        let fsinfo = fsinfo_sector.fsinfo;
        // let fsinfo = FSInfo::new(Arc::clone(&block_device));
        Self {
            bpb,
            fsinfo,
            block_device, // root_dir: (),
        }
    }
    pub fn bpb(&self) -> BiosParameterBlock {
        self.bpb
    }
    pub fn fsinfo(&self) -> FSInfo {
        self.fsinfo
    }
}

// 对 DBR 的抽象,文件系统重要信息管理,以及对文件系统的全局管理.

// 并不是 BPB 里面全部的信息,有些过时或者不重要的成员没有在里面
#[derive(Copy, Clone)]
pub(crate) struct BiosParameterBlock {
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    reserved_sectors: u16,
    fats_number: u8,     // FAT 表数,正常的为1或2
    root_entries: u16,   // 根目录的目录项数, FAT32 一直设为0
    dummy1: [u8; 9],     // 不关心的数据,比如磁道相关的这些过时的东西
    hidden_sectors: u32, // 文件系统前的隐藏扇区数,对于有分区的磁盘来说不为0
    total_sectors: u32,
    // Extended BIOS Parameter Block
    fats_sectors: u32,
    extended_flags: u16,
    fs_version: u16,
    root_dir_cluster: u32,
    fsinfo_sector_number: u16,
    backup_boot_sector: u16,
    dummy2: [u8; 15], // 不关心的数据
    volumn_id: u32,
    volume_label: [u8; 11], // 卷名, 11bytes
    fs_type_label: [u8; 8], // 文件系统类型名, 如果是FAT32就是FAT32的ascii码
}

impl BiosParameterBlock {
    pub(crate) fn fats_sectors(&self) -> u32 {
        self.fats_sectors
    }
    pub(crate) fn total_sectors(&self) -> u32 {
        self.total_sectors_32
    }
    pub(crate) fn reserved_sectors(&self) -> u32 {
        u32::from(self.reserved_sectors)
    }
    pub(crate) fn root_dir_sectors(&self) -> u32 {
        let root_dir_bytes = u32::from(self.root_entries) * DIR_ENTRY_SIZE;
        (root_dir_bytes + u32::from(self.bytes_per_sector) - 1) / u32::from(self.bytes_per_sector)
    }
    pub(crate) fn sectors_per_all_fats(&self) -> u32 {
        u32::from(self.fats_number) * self.fats_sectors()
    }
    pub(crate) fn first_data_sector(&self) -> u32 {
        let root_dir_sectors = self.root_dir_sectors();
        let fat_sectors = self.sectors_per_all_fats();
        self.reserved_sectors() + fat_sectors + root_dir_sectors
    }
    pub(crate) fn total_clusters(&self) -> u32 {
        let total_sectors = self.total_sectors();
        let first_data_sector = self.first_data_sector();
        let data_sectors = total_sectors - first_data_sector;
        data_sectors / u32::from(self.sectors_per_cluster)
    }
}

// 本文件系统实现不会改变这个起始扇区,也不能改变起始扇区,因为不具备创建文件系统,扩容等功能
#[derive(Copy, Clone)]
pub(crate) struct BootSector {
    bootjmp: [u8; 3],
    oem_name: [u8; 8],
    bpb: BiosParameterBlock,
    boot_code: [u8; 448],
    boot_sig: [u8; 2],
}

impl BootSector {
    fn initialize() {}
}

pub(crate) struct FSInfo {
    free_cluster_count: u32,
    next_free_cluster: u32,
}

impl FSInfo {
    const LEAD_SIGNATURE: u32 = 0x4161_5252;
    const STRUC_SIGNATURE: u32 = 0x6141_7272;
    const TRAIL_SIGNATURE: u32 = 0xAA55_0000;
}

// 包括 BPB 和 FSInfo 的信息
pub struct FAT32FS {
    bpb: BiosParameterBlock,
    fsinfo: FSInfo,
    pub block_device: Arc<dyn BlockDevice>,
    root_dir: Arc<RwLock<ShortDirEntry>>, // 根目录项
}

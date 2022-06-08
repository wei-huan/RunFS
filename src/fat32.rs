// 对 DBR 的抽象,文件系统重要信息管理,以及对文件系统的全局管理.

const LEAD_SIGNATURE: u32 = 0x41615252;
const STRUC_SIGNATURE: u32 = 0x61417272;
const TRAIL_SIGNATURE: u32 = 0xAA550000;

// 并不是 BPB 里面全部的信息,有些过时或者不重要的成员没有在里面
pub(crate) struct BiosParameterBlock {
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    reserved_sectors: u16,
    fats_number: u8,
    hidden_sectors: u32, // 文件系统前的隐藏扇区数,对于有分区的磁盘来说不为0
    total_sectors: u32,
    // Extended BIOS Parameter Block
    fat_size_32: u32,
    root_cluster: u32,
    fsinfo_sector_number: u16,
    backup_boot_sector: u16,
    volumn_id: u32,
    volume_label: [u8; 11],     // 卷名, 11bytes
    fs_type_label: [u8; 8],  // 文件系统类型名, 如果是FAT32就是FAT32的ascii码
}

pub(crate) struct FSInfo {
    free_cluster_count: u32,
    next_free_cluster: u32
}

// 包括 BPB 和 FSInfo 的信息
pub struct FAT32FS{
    pub block_device: Arc<dyn BlockDevice>,
    root_dir:Arc<RwLock<ShortDirEntry>>,        // 根目录项
}

// 对 BootSector, BPB抽象,文件系统重要信息管理.

use super::BlockDevice;
use crate::directory_entry::DIRENT_SZ;
use crate::error::FSError;
use crate::{MAX_CLUS_SZ, START_CLUS_ID};
use std::slice;
use std::sync::Arc;

// BPB 79 Byte
#[repr(C, packed(1))]
#[derive(Debug, Default, Copy, Clone)]
pub struct BiosParameterBlock {
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    reserved_sectors: u16,
    fats_number: u8,         // FAT 表数,正常的为1或2
    root_entries: u16,       // 根目录的目录项数, FAT32 一直设为0
    total_sectors_16: u16,   // FAT32 固定为0
    media: u8,               // 存储介质类型
    sectors_per_fat_16: u16, // FAT32 固定为0
    sectors_per_track: u16,
    heads: u16,          // 磁头数
    hidden_sectors: u32, // 文件系统前的隐藏扇区数,对于有分区的磁盘来说不为0
    total_sectors_32: u32,
    // Extended BIOS Parameter Block
    fats_sectors: u32,
    extended_flags: u16,
    fs_version: u16,
    root_dir_cluster: u32,
    fsinfo_sector: u16,
    backup_boot_sector: u16,
    dummy2: [u8; 15], // 不关心的数据
    volumn_id: u32,
    volume_label: [u8; 11], // 卷名, 11bytes
    fs_type_label: [u8; 8], // 文件系统类型名, 如果是FAT32就是FAT32的ascii码
}

impl BiosParameterBlock {
    const FAT32_MAX_CLUSTERS: u32 = 0x0FFF_FFF4;

    #[allow(unused)]
    // fn new(block_device: Arc<dyn BlockDevice>) -> Self {
    //     let bpb: BiosParameterBlock = get_info_cache(0, Arc::clone(&block_device))
    //         .read()
    //         .read(11, |bpb: &BiosParameterBlock| *bpb);
    //     bpb
    // }
    // RunFS 最先判断是否是 FAT32 类型文件系统
    fn validate_fat32(&self) -> Result<(), FSError> {
        if self.root_entries != 0
            || self.total_sectors_16 != 0
            || self.sectors_per_fat_16 != 0
            || self.fs_version != 0
            || std::str::from_utf8(&self.fs_type_label).unwrap() != "FAT32   "
        {
            println!("Unsupported filesystem: Not FAT32");
            return Err(FSError::CorruptedFileSystem);
        }
        Ok(())
    }
    // 本项目实现的 FAT32 文件系统扇区的字节数只支持范围在 512-4096 字节中二的整指数倍
    fn validate_bytes_per_sector(&self) -> Result<(), FSError> {
        if self.bytes_per_sector.count_ones() != 1 {
            println!(
                "invalid bytes_per_sector value in BPB: expected a power of two but got {}",
                self.bytes_per_sector
            );
            return Err(FSError::CorruptedFileSystem);
        }
        if self.bytes_per_sector < 512 || self.bytes_per_sector > 4096 {
            println!(
                "invalid bytes_per_sector value in BPB: expected value in range [512, 4096] but got {}",
                self.bytes_per_sector
            );
            return Err(FSError::CorruptedFileSystem);
        }
        Ok(())
    }
    // 本项目实现的 FAT32 文件系统簇的扇区数只支持二的整指数倍
    fn validate_sectors_per_cluster(&self) -> Result<(), FSError> {
        if self.sectors_per_cluster.count_ones() != 1 {
            println!(
                "invalid sectors_per_cluster value in BPB: expected a power of two but got {}",
                self.sectors_per_cluster
            );
            return Err(FSError::CorruptedFileSystem);
        }
        if self.sectors_per_cluster < 1 || self.sectors_per_cluster > 128 {
            println!(
                "invalid sectors_per_cluster value in BPB: expected value in range [1, 128] but got {}",
                self.sectors_per_cluster
            );
            return Err(FSError::CorruptedFileSystem);
        }
        Ok(())
    }
    // RunFS 实现的 FAT32 文件系统簇的字节数必须小于32KB
    fn validate_bytes_per_cluster(&self) -> Result<(), FSError> {
        let bytes_per_cluster: usize =
            usize::from(self.sectors_per_cluster) * usize::from(self.bytes_per_sector);
        if bytes_per_cluster > MAX_CLUS_SZ {
            println!(
                "invalid bytes_per_cluster value in BPB: expected value smaller than {} but got {}",
                MAX_CLUS_SZ, bytes_per_cluster
            );
            return Err(FSError::CorruptedFileSystem);
        }
        Ok(())
    }
    fn validate_reserved_sectors(&self) -> Result<(), FSError> {
        if self.reserved_sectors < 1 {
            println!(
                "invalid reserved_sectors value in BPB: {}",
                self.reserved_sectors
            );
            return Err(FSError::CorruptedFileSystem);
        }
        if self.backup_boot_sector >= self.reserved_sectors {
            println!(
                "Invalid BPB: expected backup boot-sector to be in the reserved region (sector < {}) but got sector {}",
                self.reserved_sectors, self.backup_boot_sector
            );
            return Err(FSError::CorruptedFileSystem);
        }
        if self.fsinfo_sector >= self.reserved_sectors {
            println!(
                "Invalid BPB: expected FSInfo sector to be in the reserved region (sector < {}) but got sector {}",
                self.reserved_sectors, self.fsinfo_sector
            );
            return Err(FSError::CorruptedFileSystem);
        }
        Ok(())
    }
    // RunFS 实现的 FAT32 文件系统 FAT 表数必须为1或2
    fn validate_fats(&self) -> Result<(), FSError> {
        if self.fats_number == 0 || self.fats_number > 2 {
            println!("invalid fats value in BPB: {}", self.fats_number);
            return Err(FSError::CorruptedFileSystem);
        }
        Ok(())
    }
    // RunFS 实现的 FAT32 文件系统 FAT 表数必须为1或2
    fn validate_root_entries(&self) -> Result<(), FSError> {
        if self.fats_number == 0 || self.fats_number > 2 {
            println!("invalid fats value in BPB: {}", self.fats_number);
            return Err(FSError::CorruptedFileSystem);
        }
        Ok(())
    }
    fn validate_total_sectors(&self) -> Result<(), FSError> {
        let total_sectors = self.total_sectors_32();
        let first_data_sector = self.first_data_sector();
        if self.total_sectors_32 == 0 {
            println!("Invalid BPB (total_sectors_32 should be non-zero)");
            return Err(FSError::CorruptedFileSystem);
        }
        if total_sectors <= first_data_sector {
            println!(
                "Invalid total_sectors value in BPB: expected value > {} but got {}",
                first_data_sector, total_sectors
            );
            return Err(FSError::CorruptedFileSystem);
        }
        Ok(())
    }
    fn validate_fats_sectors(&self) -> Result<(), FSError> {
        if self.fats_sectors == 0 {
            println!(
                "Invalid sectors_per_fat_32 value in FAT32 BPB: expected non-zero value but got {}",
                self.fats_sectors
            );
            return Err(FSError::CorruptedFileSystem);
        }
        Ok(())
    }
    fn validate_total_clusters(&self) -> Result<(), FSError> {
        let total_clusters = self.total_clusters();
        if total_clusters > Self::FAT32_MAX_CLUSTERS {
            println!("Invalid BPB: too many clusters {}", total_clusters);
            return Err(FSError::CorruptedFileSystem);
        }
        let total_fat_entries =
            self.fats_sectors() * u32::from(self.bytes_per_sector) * 8 / DIRENT_SZ;
        let usable_fat_entries: u32 = total_fat_entries - u32::try_from(START_CLUS_ID).unwrap();
        if usable_fat_entries < total_clusters {
            println!(
                "FAT is too small (allows allocation of {} clusters) compared to the total number of clusters ({})",
                usable_fat_entries, total_clusters
            );
        }
        Ok(())
    }
    // 验证文件系统是否是合法的FAT32类型
    #[must_use]
    pub(crate) fn validate(&self) -> Result<(), FSError> {
        self.validate_fat32()?;
        self.validate_bytes_per_sector()?;
        self.validate_sectors_per_cluster()?;
        self.validate_bytes_per_cluster()?;
        self.validate_reserved_sectors()?;
        self.validate_fats()?;
        self.validate_root_entries()?;
        self.validate_total_sectors()?;
        self.validate_fats_sectors()?;
        self.validate_total_clusters()?;
        Ok(())
    }
    pub fn bytes_per_sector(&self) -> u16 {
        self.bytes_per_sector
    }
    pub fn sectors_per_cluster(&self) -> u8 {
        self.sectors_per_cluster
    }
    pub fn fats_sectors(&self) -> u32 {
        self.fats_sectors
    }
    pub fn total_sectors_32(&self) -> u32 {
        self.total_sectors_32
    }
    pub fn reserved_sectors(&self) -> u32 {
        u32::from(self.reserved_sectors)
    }
    // FAT32 读出来的没有用
    pub fn root_dir_sectors(&self) -> u32 {
        let root_dir_bytes = u32::from(self.root_entries) * DIRENT_SZ;
        (root_dir_bytes + u32::from(self.bytes_per_sector) - 1) / u32::from(self.bytes_per_sector)
    }
    pub fn sectors_per_all_fats(&self) -> u32 {
        u32::from(self.fats_number) * self.fats_sectors()
    }
    pub fn first_data_sector(&self) -> u32 {
        let root_dir_sectors = self.root_dir_sectors();
        let fat_sectors = self.sectors_per_all_fats();
        self.reserved_sectors() + fat_sectors + root_dir_sectors
    }
    pub fn total_clusters(&self) -> u32 {
        let total_sectors = self.total_sectors_32();
        let first_data_sector = self.first_data_sector();
        let data_sectors = total_sectors - first_data_sector;
        data_sectors / u32::from(self.sectors_per_cluster)
    }
    pub fn cluster_size(&self) -> u32 {
        u32::from(self.sectors_per_cluster) * u32::from(self.bytes_per_sector)
    }
    pub fn fsinfo_sector(&self) -> u32 {
        u32::from(self.fsinfo_sector)
    }
    pub fn backup_boot_sector(&self) -> u32 {
        u32::from(self.backup_boot_sector)
    }
}

// RunFS 全程不会改变这个起始扇区,也不能改变起始扇区,因为不具备创建文件系统,扩容等功能
#[repr(C, packed(1))]
#[derive(Debug, Copy, Clone)]
pub struct BootSector {
    bootjmp: [u8; 3],
    oem_name: [u8; 8],
    pub(crate) bpb: BiosParameterBlock,
    boot_code: [u8; 420],
    boot_sig: [u8; 2],
}

impl Default for BootSector {
    fn default() -> BootSector {
        BootSector {
            bootjmp: [0; 3],
            oem_name: [0; 8],
            bpb: BiosParameterBlock::default(), // [u8; 79]
            boot_code: [0; 420],
            boot_sig: [0; 2],
        }
    }
}

impl BootSector {
    #[allow(unused)]
    fn set_bootjump(&mut self, data: &[u8]) {
        self.bootjmp = data.try_into().expect("slice with incorrect length");
    }
    #[allow(unused)]
    fn set_oem_name(&mut self, data: &[u8]) {
        self.oem_name = data.try_into().expect("slice with incorrect length");
    }
    #[allow(unused)]
    fn set_boot_code(&mut self, data: &[u8]) {
        self.boot_code = data.try_into().expect("slice with incorrect length");
    }
    #[allow(unused)]
    fn set_boot_sig(&mut self, data: &[u8]) {
        self.boot_sig = data.try_into().expect("slice with incorrect length");
    }
    // 直接通过块设备读取获得启动扇区, 只用于 RunFileSystem 创建
    pub(crate) fn directly_new(block_device: Arc<dyn BlockDevice>) -> Self {
        // println!("size of BootSector: {}", core::mem::size_of::<BootSector>());
        let boot_sector = BootSector::default();
        // 调试没问题,能够获取 512 Byte 准确数据
        let mut sector_slice = unsafe {
            slice::from_raw_parts_mut(
                (&boot_sector as *const BootSector) as *mut u8,
                core::mem::size_of::<BootSector>(),
            )
        };
        block_device.read_block(0, sector_slice);
        boot_sector
    }
    // pub(crate) fn new(block_device: Arc<dyn BlockDevice>) -> Self {
    //     let boot_sector: BootSector = get_info_cache(0, Arc::clone(&block_device))
    //         .read()
    //         .read(0, |bs: &BootSector| *bs);
    //     boot_sector
    // }
    pub(crate) fn validate(&self) -> Result<(), FSError> {
        if self.boot_sig != [0x55, 0xAA] {
            println!(
                "Invalid boot sector signature: expected [0x55, 0xAA] but got {:?}",
                self.boot_sig
            );
            return Err(FSError::CorruptedFileSystem);
        }
        if self.bootjmp[0] != 0xEB && self.bootjmp[0] != 0xE9 {
            println!(
                "Unknown opcode {:x} in bootjmp boot sector field",
                self.bootjmp[0]
            );
        }
        self.bpb.validate()?;
        Ok(())
    }
}

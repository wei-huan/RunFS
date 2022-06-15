use bitflags::bitflags;

const START_YEAR: u32 = 1980;

pub(crate) const DIRENT_SZ: u32 = 32; // 目录项字节数
pub(crate) const DIR_ENTRY_DELETED_FLAG: u8 = 0xE5;
pub(crate) const SHORT_FILE_NAME_LEN: usize = 8;
pub(crate) const SHORT_FILE_EXT_LEN: usize = 3;

bitflags! {
    /// A FAT file attributes.
    /// 目录项 ATTRIBUTE 字节最高两位是保留不用的
    #[derive(Default)]
    pub struct FileAttributes: u8 {
        const READ_ONLY  = 0x01;
        const HIDDEN     = 0x02;
        const SYSTEM     = 0x04;
        const VOLUME_ID  = 0x08;
        const DIRECTORY  = 0x10;
        const ARCHIVE    = 0x20;    // 确定是否需要写回外存,在文件的创建,调整,重命名时需要置位
        const LONG_NAME  = Self::READ_ONLY.bits | Self::HIDDEN.bits
                        | Self::SYSTEM.bits | Self::VOLUME_ID.bits;
        const LONG_NAME_MASK = Self::READ_ONLY.bits | Self::HIDDEN.bits
                            | Self::SYSTEM.bits | Self::VOLUME_ID.bits
                            | Self::DIRECTORY.bits | Self::ARCHIVE.bits;
    }
}

pub const LAST_LONG_ENTRY: u8 = 0x40;

// 短目录项,也适用于当前目录项和上级目录项
#[repr(C, packed(1))]
#[derive(Clone, Debug, Default)]
pub struct ShortDirectoryEntry {
    name: [u8; SHORT_FILE_NAME_LEN], // 删除时第0位为0xE5，未使用时为0x00. 有多余可以用0x20填充
    extension: [u8; SHORT_FILE_EXT_LEN],
    attribute: FileAttributes, //可以用于判断是目录还是文件或者卷标
    os_reserved: u8,
    creation_tenths: u8,
    creation_time: u16,
    creation_date: u16,
    last_acc_date: u16,
    cluster_high: u16,
    modification_time: u16,
    modification_date: u16,
    cluster_low: u16,
    size: u32,
}

impl ShortDirectoryEntry {
    pub fn new(
        name: [u8; SHORT_FILE_NAME_LEN],
        extension: [u8; SHORT_FILE_EXT_LEN],
        attribute: FileAttributes,
    ) -> Self {
        Self {
            name,
            extension,
            attribute,
            ..Self::default()
        }
    }
    pub fn attribute(&self) -> FileAttributes {
        self.attribute
    }
    pub fn is_dir(&self) -> bool {
        self.attribute.contains(FileAttributes::DIRECTORY)
    }
    pub fn is_volume(&self) -> bool {
        self.attribute.contains(FileAttributes::VOLUME_ID)
    }
    pub fn is_deleted(&self) -> bool {
        self.name[0] == DIR_ENTRY_DELETED_FLAG
    }
    pub fn set_deleted(&mut self) {
        self.name[0] = DIR_ENTRY_DELETED_FLAG;
    }
    pub fn is_empty(&self) -> bool {
        self.name[0] == 0x00
    }
    pub fn is_file(&self) -> bool {
        (!self.is_dir()) && (!self.is_volume())
    }
    pub fn get_creation_time(&self) -> (u32, u32, u32, u32, u32, u32, u64) {
        // year-month-day-Hour-min-sec-long_sec
        let year: u32 = ((self.creation_date & 0xFE00) >> 9) as u32 + 1980;
        let month: u32 = ((self.creation_date & 0x01E0) >> 5) as u32;
        let day: u32 = (self.creation_date & 0x001F) as u32;
        let hour: u32 = ((self.creation_time & 0xF800) >> 11) as u32;
        let min: u32 = ((self.creation_time & 0x07E0) >> 5) as u32;
        let sec: u32 = ((self.creation_time & 0x001F) << 1) as u32; // 秒数需要*2
        let long_sec: u64 =
            ((((year - 1980) * 365 + month * 30 + day) * 24 + hour) * 3600 + min * 60 + sec) as u64;
        (year, month, day, hour, min, sec, long_sec)
    }

    pub fn modification_time(&self) -> (u32, u32, u32, u32, u32, u32, u64) {
        // year-month-day-Hour-min-sec
        let year: u32 = ((self.modification_date & 0xFE00) >> 9) as u32 + START_YEAR;
        let month: u32 = ((self.modification_date & 0x01E0) >> 5) as u32;
        let day: u32 = (self.modification_date & 0x001F) as u32;
        let hour: u32 = ((self.modification_time & 0xF800) >> 11) as u32;
        let min: u32 = ((self.modification_time & 0x07E0) >> 5) as u32;
        let sec: u32 = ((self.modification_time & 0x001F) << 1) as u32; // 秒数需要*2
        let long_sec: u64 = ((((year - START_YEAR) * 365 + month * 30 + day) * 24 + hour) * 3600
            + min * 60
            + sec) as u64;
        (year, month, day, hour, min, sec, long_sec)
    }

    pub fn accessed_time(&self) -> (u32, u32, u32, u32, u32, u32, u64) {
        // year-month-day-Hour-min-sec
        let year: u32 = ((self.last_acc_date & 0xFE00) >> 9) as u32 + START_YEAR;
        let month: u32 = ((self.last_acc_date & 0x01E0) >> 5) as u32;
        let day: u32 = (self.last_acc_date & 0x001F) as u32;
        let hour: u32 = 0;
        let min: u32 = 0;
        let sec: u32 = 0; // 没有相关信息，默认0
        let long_sec: u64 = ((((year - START_YEAR) * 365 + month * 30 + day) * 24 + hour) * 3600
            + min * 60
            + sec) as u64;
        (year, month, day, hour, min, sec, long_sec)
    }
    // 获取文件起始簇号
    pub fn first_cluster(&self) -> u32 {
        ((self.cluster_high as u32) << 16) + (self.cluster_low as u32)
        // let n = ((self.cluster_high as u32) << 16) + (self.cluster_low as u32);
        // if n == 0 {
        //     None
        // } else {
        //     Some(n)
        // }
    }
    // 设置文件起始簇号
    pub fn set_first_cluster(&mut self, cluster: u32) {
        self.cluster_high = ((cluster & 0xFFFF0000) >> 16) as u16;
        self.cluster_low = (cluster & 0x0000FFFF) as u16;
    }
    // pub fn set_first_cluster(&mut self, cluster: Option<u32>) {
    //     let n = cluster.unwrap_or(0);
    //     self.cluster_high = (n >> 16) as u16;
    //     self.cluster_low = (n & 0x00FF) as u16;
    // }
    pub fn size(&self) -> Option<u32> {
        if self.is_file() {
            Some(self.size)
        } else {
            None
        }
    }
    pub fn set_size(&mut self, size: u32) {
        self.size = size;
    }
    // 获取短文件名
    pub fn name(&self) -> String {
        let mut name: String = String::new();
        for i in 0..8 {
            // 记录文件名
            if self.name[i] == 0x20 {
                break;
            } else {
                name.push(self.name[i] as char);
            }
        }
        for i in 0..3 {
            // 记录扩展名
            if self.extension[i] == 0x20 {
                break;
            } else {
                if i == 0 {
                    name.push('.');
                }
                name.push(self.extension[i] as char);
            }
        }
        name
    }
    /* 计算校验和 */
    pub fn checksum(&self) -> u8 {
        let mut name_buff: [u8; 11] = [0u8; 11];
        let mut sum: u8 = 0;
        for i in 0..8 {
            name_buff[i] = self.name[i];
        }
        for i in 0..3 {
            name_buff[i + 8] = self.extension[i];
        }
        for i in 0..11 {
            if (sum & 1) != 0 {
                sum = 0x80 + (sum >> 1) + name_buff[i];
            } else {
                sum = (sum >> 1) + name_buff[i];
            }
        }
        sum
    }
    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            core::slice::from_raw_parts(
                self as *const _ as usize as *const u8,
                DIRENT_SZ.try_into().unwrap(),
            )
        }
    }
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(
                self as *mut _ as usize as *mut u8,
                DIRENT_SZ.try_into().unwrap(),
            )
        }
    }
}

// 长目录项, 一般来说现在的 OS 无论创建的文件或目录名字是否超出短目录项要求都会在短目录项前添加长目录项
#[repr(C, packed(1))]
#[derive(Clone, Debug, Default)]
pub struct LongDirectoryEntry {
    // use Unicode !!!
    // 如果是该文件的最后一个长文件名目录项，
    // 则将该目录项的序号与 0x40 进行“或（OR）运算”的结果写入该位置。
    // 长文件名要有\0
    order: u8,                 // 从1开始计数, 删除时为0xE5
    name1: [u8; 10],           // 5characters
    attribute: FileAttributes, // should be 0x0F
    type_: u8,
    checksum: u8,
    name2: [u8; 12], // 6characters
    zero: [u8; 2],
    name3: [u8; 4], // 2characters
}

impl LongDirectoryEntry {
    pub fn new(order: u8, checksum: u8) -> Self {
        Self {
            order,
            checksum,
            attribute: FileAttributes::LONG_NAME,
            ..Self::default()
        }
    }
    pub fn order(&self) -> u8 {
        self.order
    }
    pub fn attribute(&self) -> FileAttributes {
        self.attribute
    }
    pub fn is_empty(&self) -> bool {
        self.order == 0x00
    }
    pub fn is_deleted(&self) -> bool {
        self.order == DIR_ENTRY_DELETED_FLAG
    }
    pub fn set_deleted(&mut self) {
        self.order = DIR_ENTRY_DELETED_FLAG;
    }
    pub fn is_last(&self) -> bool {
        (self.order & LAST_LONG_ENTRY) != 0
    }
    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            core::slice::from_raw_parts(
                self as *const _ as usize as *const u8,
                DIRENT_SZ.try_into().unwrap(),
            )
        }
    }
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(
                self as *mut _ as usize as *mut u8,
                DIRENT_SZ.try_into().unwrap(),
            )
        }
    }
    pub fn checksum(&self) -> u8 {
        self.checksum
    }
}

const VOLUME_NAME_LEN: usize = 11;

// 卷标目录项
#[derive(Default)]
struct VolumeLabelEntry {
    name: [u8; VOLUME_NAME_LEN], // 删除时第0位为0xE5，未使用时为0x00. 有多余可以用0x20填充
    attribute: FileAttributes,   // 删除时为0xE5
    os_reserved: u8,
    entry_reserved_1: [u8; 9],
    modification_time: u16,
    modification_date: u16,
    entry_reserved_2: [u8; 6],
}

impl VolumeLabelEntry {
    pub fn new(name: [u8; VOLUME_NAME_LEN], attribute: FileAttributes) -> Self {
        Self {
            name,
            attribute,
            ..Self::default()
        }
    }
    // 获取卷名
    pub fn name(&self) -> String {
        let mut name: String = String::new();
        for i in 0..VOLUME_NAME_LEN {
            // 记录文件名
            if self.name[i] == 0x20 {
                break;
            } else {
                name.push(self.name[i] as char);
            }
        }
        name
    }
    pub fn attribute(&self) -> FileAttributes {
        self.attribute
    }
    pub fn modification_time(&self) -> (u32, u32, u32, u32, u32, u32, u64) {
        // year-month-day-Hour-min-sec
        let year: u32 = ((self.modification_date & 0xFE00) >> 9) as u32 + START_YEAR;
        let month: u32 = ((self.modification_date & 0x01E0) >> 5) as u32;
        let day: u32 = (self.modification_date & 0x001F) as u32;
        let hour: u32 = ((self.modification_time & 0xF800) >> 11) as u32;
        let min: u32 = ((self.modification_time & 0x07E0) >> 5) as u32;
        let sec: u32 = ((self.modification_time & 0x001F) << 1) as u32; // 秒数需要*2
        let long_sec: u64 = ((((year - START_YEAR) * 365 + month * 30 + day) * 24 + hour) * 3600
            + min * 60
            + sec) as u64;
        (year, month, day, hour, min, sec, long_sec)
    }
}

/// 目录项抽象
pub enum DirectoryEntry {
    LongDirectoryEntry(LongDirectoryEntry),
    ShortDirectoryEntry(ShortDirectoryEntry),
    VolumeLabelEntry(VolumeLabelEntry),
}

impl DirectoryEntry {}

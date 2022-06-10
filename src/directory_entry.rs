pub(crate) const DIRENT_SZ: u32 = 32; // 目录项字节数

// 目录项 ATTRIBUTE 字节最高两位是保留不用的
pub const ATTR_READ_ONLY: u8 = 0x01;
pub const ATTR_HIDDEN: u8 = 0x02;
pub const ATTR_SYSTEM: u8 = 0x04;
pub const ATTR_VOLUME_ID: u8 = 0x08;
pub const ATTR_DIRECTORY: u8 = 0x10;
pub const ATTR_ARCHIVE: u8 = 0x20;
pub const ATTR_LONG_NAME: u8 = ATTR_READ_ONLY | ATTR_HIDDEN | ATTR_SYSTEM | ATTR_VOLUME_ID;
pub const ATTR_LONG_NAME_MASK: u8 =
    ATTR_READ_ONLY | ATTR_HIDDEN | ATTR_SYSTEM | ATTR_VOLUME_ID | ATTR_DIRECTORY | ATTR_ARCHIVE;

pub const LAST_LONG_ENTRY: u8 = 0x40;

/// 目录项抽象
enum FileDirectoryEntry {
    LongDirectoryEntry(LongDirectoryEntry),
    ShortDirectoryEntry(ShortDirectoryEntry),
    VolumeLabelEntry(VolumeLabelEntry),
}

// 短目录项,也适用于当前目录项和上级目录项
pub(crate) struct ShortDirectoryEntry {
    name: [u8; 8], // 删除时第0位为0xE5，未使用时为0x00. 有多余可以用0x20填充
    extension: [u8; 3],
    attribute: u8, //可以用于判断是目录还是文件或者卷标
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
    pub fn is_dir(&self) -> bool {
        self.attribute == ATTR_DIRECTORY
    }
    pub fn is_deleted(&self) -> bool {
        self.name[0] == 0xE5
    }
    pub fn is_empty(&self) -> bool {
        self.name[0] == 0x00
    }
    pub fn is_file(&self) -> bool {
        (self.attribute != ATTR_DIRECTORY) && (self.attribute != ATTR_VOLUME_ID)
    }
    pub fn is_long(&self) -> bool {
        self.attribute == ATTR_LONG_NAME
    }
    pub fn attribute(&self) -> u8 {
        self.attribute
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

    pub fn get_modification_time(&self) -> (u32, u32, u32, u32, u32, u32, u64) {
        // year-month-day-Hour-min-sec
        let year: u32 = ((self.modification_date & 0xFE00) >> 9) as u32 + 1980;
        let month: u32 = ((self.modification_date & 0x01E0) >> 5) as u32;
        let day: u32 = (self.modification_date & 0x001F) as u32;
        let hour: u32 = ((self.modification_time & 0xF800) >> 11) as u32;
        let min: u32 = ((self.modification_time & 0x07E0) >> 5) as u32;
        let sec: u32 = ((self.modification_time & 0x001F) << 1) as u32; // 秒数需要*2
        let long_sec: u64 =
            ((((year - 1980) * 365 + month * 30 + day) * 24 + hour) * 3600 + min * 60 + sec) as u64;
        (year, month, day, hour, min, sec, long_sec)
    }

    pub fn get_accessed_time(&self) -> (u32, u32, u32, u32, u32, u32, u64) {
        // year-month-day-Hour-min-sec
        let year: u32 = ((self.last_acc_date & 0xFE00) >> 9) as u32 + 1980;
        let month: u32 = ((self.last_acc_date & 0x01E0) >> 5) as u32;
        let day: u32 = (self.last_acc_date & 0x001F) as u32;
        let hour: u32 = 0;
        let min: u32 = 0;
        let sec: u32 = 0; // 没有相关信息，默认0
        let long_sec: u64 =
            ((((year - 1980) * 365 + month * 30 + day) * 24 + hour) * 3600 + min * 60 + sec) as u64;
        (year, month, day, hour, min, sec, long_sec)
    }
    /*获取文件起始簇号*/
    pub fn first_cluster(&self) -> u32 {
        ((self.cluster_high as u32) << 16) + (self.cluster_low as u32)
    }
    /*获取短文件名*/
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
    /* 设置当前文件的大小 */
    // 簇的分配和回收实际要对FAT表操作
    pub fn set_size(&mut self, size: u32) {
        self.size = size;
    }

    pub fn get_size(&self) -> u32 {
        self.size
    }
    /* 设置文件起始簇 */
    pub fn set_first_cluster(&mut self, cluster: u32) {
        self.cluster_high = ((cluster & 0xFFFF0000) >> 16) as u16;
        self.cluster_low = (cluster & 0x0000FFFF) as u16;
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
struct LongDirectoryEntry {
    // use Unicode !!!
    // 如果是该文件的最后一个长文件名目录项，
    // 则将该目录项的序号与 0x40 进行“或（OR）运算”的结果写入该位置。
    // 长文件名要有\0
    order: u8,       // 删除时为0xE5
    name1: [u8; 10], // 5characters
    attribute: u8,   // should be 0x0F
    type_: u8,
    check_sum: u8,
    name2: [u8; 12], // 6characters
    zero: [u8; 2],
    name3: [u8; 4], // 2characters
}

impl LongDirectoryEntry {
    pub fn attribute(&self) -> u8 {
        self.attribute
    }
    pub fn is_empty(&self) -> bool {
        self.order == 0x00
    }
    pub fn is_last(&self) -> bool {
        (self.order & LAST_LONG_ENTRY) > 0
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
    pub fn order(&self) -> u8 {
        self.order
    }
    pub fn checksum(&self) -> u8 {
        self.check_sum
    }
}

// 卷标目录项
struct VolumeLabelEntry {
    name: [u8; 11], // 删除时第0位为0xE5，未使用时为0x00. 有多余可以用0x20填充
    attribute: u8,  // 删除时为0xE5
    os_reserved: u8,
    entry_reserved_1: [u8; 9],
    modification_time: u16,
    modification_date: u16,
    entry_reserved_2: [u8; 6],
}

impl VolumeLabelEntry {
    /*获取卷名*/
    pub fn name(&self) -> String {
        let mut name: String = String::new();
        for i in 0..11 {
            // 记录文件名
            if self.name[i] == 0x20 {
                break;
            } else {
                name.push(self.name[i] as char);
            }
        }
        name
    }
}

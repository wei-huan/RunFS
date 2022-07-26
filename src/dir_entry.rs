use super::RunFileSystem;
#[cfg(not(feature = "std"))]
use alloc::{string::String, sync::Arc};
use bitflags::bitflags;
use spin::RwLock;
#[cfg(feature = "std")]
use std::sync::Arc;

const START_YEAR: u32 = 1980;

pub(crate) const DIRENT_SZ: usize = 32; // 目录项字节数
pub(crate) const DIR_ENTRY_DELETED_FLAG: u8 = 0xE5;
pub(crate) const SHORT_FILE_NAME_LEN: usize = 8;
pub(crate) const SHORT_FILE_EXT_LEN: usize = 3;
pub(crate) const SHORT_FILE_NAME_PADDING: u8 = b' ';
pub(crate) const SHORT_NAME_LEN: usize = SHORT_FILE_NAME_LEN + SHORT_FILE_EXT_LEN;
pub(crate) const LONG_NAME_LEN: usize = 13;

bitflags! {
    /// A FAT file attributes.
    /// 目录项 ATTRIBUTE 字节最高两位是保留不用的
    #[derive(Default)]
    #[repr(C, packed(1))]
    pub struct FileAttributes: u8 {
        const FILE       = 0x00;
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

/// 短目录项,也适用于当前目录项和上级目录项
/// 短目录项实际就是文件和文件夹的句柄

#[repr(packed)]
#[derive(Copy, Clone, Default)]
pub struct ShortDirectoryEntry {
    name: [u8; SHORT_FILE_NAME_LEN], // 删除时第0位为0xE5，未使用时为0x00. 有多余可以用0x20填充
    extension: [u8; SHORT_FILE_EXT_LEN],
    attribute: FileAttributes, //可以用于判断是目录还是文件或者卷标
    _os_reserved: u8,
    _creation_tenths: u8,
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
        first_cluster: u32,
    ) -> Self {
        Self {
            name,
            extension,
            attribute,
            cluster_low: (first_cluster & 0x0000FFFF) as u16,
            cluster_high: ((first_cluster & 0xFFFF0000) >> 16) as u16,
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
    pub fn is_free(&self) -> bool {
        self.is_deleted() || self.is_empty()
    }
    pub fn is_file(&self) -> bool {
        (!self.is_dir()) && (!self.is_volume())
    }
    pub fn is_short(&self) -> bool {
        !self.attribute.contains(FileAttributes::LONG_NAME)
    }
    pub fn get_creation_time(&self) -> (u32, u32, u32, u32, u32, u32, u64) {
        // year-month-day-Hour-min-sec-long_sec
        let year: u32 = ((self.creation_date & 0xFE00) >> 9) as u32 + 1980;
        let month: u32 = ((self.creation_date & 0x01E0) >> 5) as u32;
        let day: u32 = (self.creation_date & 0x001F) as u32;
        let hour: u32 = ((self.creation_time & 0xF800) >> 11) as u32;
        let min: u32 = ((self.creation_time & 0x07E0) >> 5) as u32;
        let sec: u32 = ((self.creation_time & 0x001F) << 1) as u32; // 秒数需要*2
        let long_sec: u64 = ((((year - START_YEAR) * 365 + month * 30 + day) * 24 + hour) * 3600
            + min * 60
            + sec) as u64;
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
    }
    // 设置文件起始簇号
    pub fn set_first_cluster(&mut self, cluster: u32) {
        self.cluster_high = ((cluster & 0xFFFF0000) >> 16) as u16;
        self.cluster_low = (cluster & 0x0000FFFF) as u16;
    }
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
    // 获取短文件名,短文件名默认都是大写
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
    /// 计算校验和
    pub fn checksum(&self) -> u8 {
        let mut name_buff: [u8; SHORT_NAME_LEN] = [0x20u8; SHORT_NAME_LEN];
        let mut sum: u8 = 0;
        let mut temp: u16;
        for i in 0..SHORT_FILE_NAME_LEN {
            name_buff[i] = self.name[i];
        }
        for i in 0..SHORT_FILE_EXT_LEN {
            name_buff[i + SHORT_FILE_NAME_LEN] = self.extension[i];
        }
        for i in 0..SHORT_NAME_LEN {
            if (sum & 1) != 0 {
                temp = 0x80 + (sum >> 1) as u16 + name_buff[i] as u16;
                sum = (temp & 0xFF) as u8;
            } else {
                temp = (sum >> 1) as u16 + name_buff[i] as u16;
                sum = (temp & 0xFF) as u8;
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
    /// 获取文件偏移量所在的簇和偏移
    pub fn pos(&self, offset: usize, fs: &Arc<RwLock<RunFileSystem>>) -> (Option<usize>, usize) {
        let runfs = fs.read();
        let bytes_per_cluster = runfs.bpb().cluster_size() as usize;
        let cluster_index = offset / bytes_per_cluster;
        let current_cluster = runfs
            .fat_manager_modify()
            .search_cluster(self.first_cluster() as usize, cluster_index);
        // println!("first_cluster: {}", self.first_cluster() as usize);
        (current_cluster, offset % bytes_per_cluster)
    }
    /// 以偏移量读取文件, 返回实际读取的长度
    pub fn read_at(
        &self,
        offset: usize,
        buf: &mut [u8],
        runfs: &Arc<RwLock<RunFileSystem>>,
    ) -> usize {
        // println!("1-0-0-0");
        let cluster_size = runfs.read().bpb().cluster_size();
        let mut current_offset = offset;
        let mut size = self.size as usize;
        // 计算文件夹占用的空间
        if self.is_dir() {
            size = cluster_size
                * runfs
                    .read()
                    .fat_manager_modify()
                    .count_clusters(self.first_cluster() as usize);
        }
        // println!("read_at size = {}", size);
        // println!("1-0-0-1");
        let offset_end_pos = (offset + buf.len()).min(size);
        // println!(
        //     "read_at current_offset = {}; offset_end_pos = {}",
        //     current_offset, offset_end_pos
        // );
        // println!("1-0-0-2");
        if current_offset >= offset_end_pos {
            return 0;
        }
        let (cluster_id, _) = self.pos(offset, runfs);
        let mut current_cluster = match cluster_id {
            None => return 0,
            Some(id) => id,
        };
        // println!("current_cluster: {}", current_cluster);
        let mut read_size = 0usize;
        loop {
            // println!("1-0-0-3");
            // 将偏移量向上对齐簇大小
            let mut current_cluster_end_pos = (current_offset / cluster_size + 1) * cluster_size;
            current_cluster_end_pos = current_cluster_end_pos.min(offset_end_pos);
            // println!("current_cluster_end_pos = {}", current_cluster_end_pos);
            // 开始读
            let cluster_read_size = current_cluster_end_pos - current_offset;
            let offset_in_cluster = current_offset % cluster_size;
            let dst = &mut buf[read_size..read_size + cluster_read_size];
            for i in 0..cluster_read_size {
                runfs.read().data_manager_modify().read_cluster_at(
                    current_cluster,
                    offset_in_cluster + i,
                    |data: &u8| {
                        dst[i] = *data;
                    },
                );
            }
            // println!("1-0-0-4");
            // 更新读取长度
            read_size += cluster_read_size;
            if current_cluster_end_pos == offset_end_pos {
                break;
            }
            // 更新索引参数
            current_offset = current_cluster_end_pos;
            let next_cluster = runfs
                .read()
                .fat_manager_modify()
                .next_cluster(current_cluster);
            current_cluster = match next_cluster {
                None => break, // 没有下一个簇
                Some(id) => id,
            };
        }
        // println!("read_size: {}", read_size);
        // println!("1-0-0-5");
        read_size
    }

    /// 以偏移量写文件
    pub fn write_at(&self, offset: usize, buf: &[u8], runfs: &Arc<RwLock<RunFileSystem>>) -> usize {
        let cluster_size = runfs.read().bpb().cluster_size() as usize;
        let mut current_offset = offset;
        let capacity = cluster_size
            * runfs
                .read()
                .fat_manager_modify()
                .count_clusters(self.first_cluster() as usize) as usize;
        // println!("write_at size = {}", capacity);
        let offset_end_pos = (offset + buf.len()).min(capacity);
        if current_offset >= offset_end_pos {
            return 0;
        }
        // println!(
        //     "write_at current_offset = {}; offset_end_pos = {}",
        //     current_offset, offset_end_pos
        // );
        let (cluster_id, _) = self.pos(offset, runfs);
        let mut current_cluster = match cluster_id {
            None => return 0,
            Some(id) => id,
        };
        // println!("current_cluster = {}", current_cluster);
        let mut write_size = 0usize;
        loop {
            // 将偏移量向上对齐簇大小
            let mut current_cluster_end_pos = (current_offset / cluster_size + 1) * cluster_size;
            current_cluster_end_pos = current_cluster_end_pos.min(offset_end_pos);
            // 开始写
            let cluster_write_size = current_cluster_end_pos - current_offset;
            let offset_in_cluster = current_offset % cluster_size;
            let src = &buf[write_size..write_size + cluster_write_size];
            for i in 0..cluster_write_size {
                runfs.read().data_manager_modify().write_cluster_at(
                    current_cluster,
                    offset_in_cluster + i,
                    |data: &mut u8| {
                        *data = src[i];
                    },
                );
            }
            // 更新写入长度
            write_size += cluster_write_size;
            if current_cluster_end_pos == offset_end_pos {
                break;
            }
            // 更新索引参数
            current_offset = current_cluster_end_pos;
            let next_cluster = runfs
                .read()
                .fat_manager_modify()
                .next_cluster(current_cluster);
            current_cluster = match next_cluster {
                None => break, // 没有下一个簇
                Some(id) => id,
            };
        }
        write_size
    }
}

/// 长目录项, 一般来说现在的 OS 无论创建的文件或目录的名字是否超
/// 出短目录项要求都会在短目录项前添加长目录项
/// , packed(1)
#[repr(packed)]
#[derive(Default)]
pub struct LongDirectoryEntry {
    order: u8,                 // 从1开始计数, 删除时为0xE5
    name1: [u16; 5],           // 5characters
    attribute: FileAttributes, // should be 0x0F
    _type: u8,
    checksum: u8,
    name2: [u16; 6], // 6characters
    _zero: [u8; 2],
    name3: [u16; 2], // 2characters
}

impl LongDirectoryEntry {
    pub fn new(name: [u16; LONG_NAME_LEN], order: u8, checksum: u8) -> Self {
        let mut entry = Self {
            order,
            checksum,
            attribute: FileAttributes::LONG_NAME,
            ..Self::default()
        };
        entry.name_from_slice(&name);
        entry
    }
    pub fn name_from_slice(&mut self, long_entry_name: &[u16; LONG_NAME_LEN]) {
        // self.name1.copy_from_slice(&lfn_part[0..5]);
        // self.name2.copy_from_slice(&lfn_part[5..11]);
        // self.name3.copy_from_slice(&lfn_part[11..13]);

        self.name1[0] = long_entry_name[0];
        self.name1[1] = long_entry_name[1];
        self.name1[2] = long_entry_name[2];
        self.name1[3] = long_entry_name[3];
        self.name1[4] = long_entry_name[4];
        self.name2[0] = long_entry_name[5];
        self.name2[1] = long_entry_name[6];
        self.name2[2] = long_entry_name[7];
        self.name2[3] = long_entry_name[8];
        self.name2[4] = long_entry_name[9];
        self.name2[5] = long_entry_name[10];
        self.name3[0] = long_entry_name[11];
        self.name3[1] = long_entry_name[12];
    }
    pub fn name_to_array(&self) -> [u16; LONG_NAME_LEN] {
        let mut long_entry_name = [0u16; LONG_NAME_LEN];
        // long_entry_name[0..5].copy_from_slice(&self.name1);
        // long_entry_name[5..11].copy_from_slice(&self.name2);
        // long_entry_name[11..13].copy_from_slice(&self.name3);
        long_entry_name[0] = self.name1[0];
        long_entry_name[1] = self.name1[1];
        long_entry_name[2] = self.name1[2];
        long_entry_name[3] = self.name1[3];
        long_entry_name[4] = self.name1[4];

        long_entry_name[5] = self.name2[0];
        long_entry_name[6] = self.name2[1];
        long_entry_name[7] = self.name2[2];
        long_entry_name[8] = self.name2[3];
        long_entry_name[9] = self.name2[4];
        long_entry_name[10] = self.name2[5];

        long_entry_name[11] = self.name3[0];
        long_entry_name[12] = self.name3[1];
        long_entry_name
    }

    /// 长文件名转字符串
    pub fn name_format(&self) -> String {
        let mut name = String::new();
        let mut c: u8;
        for i in 0..5 {
            c = self.name1[i] as u8;
            if c == 0 {
                return name;
            }
            name.push(c as char);
        }
        for i in 0..6 {
            c = self.name2[i] as u8;
            if c == 0 {
                return name;
            }
            name.push(c as char);
        }
        for i in 0..2 {
            c = self.name3[i] as u8;
            if c == 0 {
                return name;
            }
            name.push(c as char);
        }
        return name;
    }
    pub fn order(&self) -> u8 {
        self.order
    }
    pub fn raw_order(&self) -> u8 {
        self.order ^ LAST_LONG_ENTRY
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
    pub fn is_free(&self) -> bool {
        self.is_empty() || self.is_deleted()
    }
    pub fn is_long(&self) -> bool {
        self.attribute.contains(FileAttributes::LONG_NAME)
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

// const VOLUME_NAME_LEN: usize = 11;

// 卷标目录项
// #[derive(Default)]
// struct VolumeLabelEntry {
//     name: [u8; VOLUME_NAME_LEN], // 删除时第0位为0xE5，未使用时为0x00. 有多余可以用0x20填充
//     attribute: FileAttributes,   // 删除时为0xE5
//     os_reserved: u8,
//     entry_reserved_1: [u8; 9],
//     modification_time: u16,
//     modification_date: u16,
//     entry_reserved_2: [u8; 6],
// }

// impl VolumeLabelEntry {
//     pub fn new(name: [u8; VOLUME_NAME_LEN], attribute: FileAttributes) -> Self {
//         Self {
//             name,
//             attribute,
//             ..Self::default()
//         }
//     }
//     // 获取卷名
//     pub fn name(&self) -> String {
//         let mut name: String = String::new();
//         for i in 0..VOLUME_NAME_LEN {
//             // 记录文件名
//             if self.name[i] == 0x20 {
//                 break;
//             } else {
//                 name.push(self.name[i] as char);
//             }
//         }
//         name
//     }
//     pub fn attribute(&self) -> FileAttributes {
//         self.attribute
//     }
//     pub fn modification_time(&self) -> (u32, u32, u32, u32, u32, u32, u64) {
//         // year-month-day-Hour-min-sec
//         let year: u32 = ((self.modification_date & 0xFE00) >> 9) as u32 + START_YEAR;
//         let month: u32 = ((self.modification_date & 0x01E0) >> 5) as u32;
//         let day: u32 = (self.modification_date & 0x001F) as u32;
//         let hour: u32 = ((self.modification_time & 0xF800) >> 11) as u32;
//         let min: u32 = ((self.modification_time & 0x07E0) >> 5) as u32;
//         let sec: u32 = ((self.modification_time & 0x001F) << 1) as u32; // 秒数需要*2
//         let long_sec: u64 = ((((year - START_YEAR) * 365 + month * 30 + day) * 24 + hour) * 3600
//             + min * 60
//             + sec) as u64;
//         (year, month, day, hour, min, sec, long_sec)
//     }
// }

// /// 目录项抽象
// pub enum DirectoryEntry {
//     LongDirectoryEntry(LongDirectoryEntry),
//     ShortDirectoryEntry(ShortDirectoryEntry),
//     VolumeLabelEntry(VolumeLabelEntry),
// }

// impl DirectoryEntry {}

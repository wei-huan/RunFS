/// 虚拟文件系统, 将实际文件系统抽象成满足文件,文件夹创建读写删除功能的抽象文件系统
use super::{
    FSError, FileAttributes, LongDirectoryEntry, RunFileSystem, ShortDirectoryEntry, DIRENT_SZ,
    LAST_LONG_ENTRY, LONG_NAME_LEN, SHORT_FILE_EXT_LEN, SHORT_FILE_NAME_LEN,
    SHORT_FILE_NAME_PADDING, SHORT_NAME_LEN,
};
#[cfg(not(feature = "std"))]
use crate::println;
#[cfg(not(feature = "std"))]
use alloc::{string::String, sync::Arc, vec::Vec};
use spin::RwLock;
#[cfg(feature = "std")]
use std::sync::Arc;

/// 将长文件名拆分
pub fn long_name_split(name: &str) -> Vec<[u16; LONG_NAME_LEN]> {
    let name_u16: Vec<u16> = name.encode_utf16().collect();
    let chunks = name_u16.as_slice().chunks_exact(LONG_NAME_LEN);
    let mut name_vec: Vec<[u16; LONG_NAME_LEN]> = Vec::new();
    let remainder = chunks.remainder();
    let remainder_len = remainder.len();
    for data in chunks {
        name_vec.push(data.try_into().unwrap());
    }
    if remainder_len > 0 {
        let mut last: [u16; LONG_NAME_LEN] = [0xFFFF; LONG_NAME_LEN];
        last[0..remainder_len].copy_from_slice(remainder);
        last[remainder_len] = 0x0000;
        name_vec.push(last);
    }
    name_vec
}

fn copy_short_name_part(dst: &mut [u8], src: &str) -> (usize, bool, bool) {
    let mut dst_pos = 0;
    let mut lossy_conv = false;
    for c in src.chars() {
        if dst_pos == dst.len() {
            // result buffer is full
            return (dst_pos, false, lossy_conv);
        }
        // Make sure character is allowed in 8.3 name
        #[rustfmt::skip]
        let fixed_c = match c {
            // strip spaces and dots
            ' ' | '.' => {
                lossy_conv = true;
                continue;
            },
            // copy allowed characters
            'A'..='Z' | 'a'..='z' | '0'..='9'
            | '!' | '#' | '$' | '%' | '&' | '\'' | '(' | ')' | '-' | '@' | '^' | '_' | '`' | '{' | '}' | '~' => c,
            // replace disallowed characters by underscore
            _ => '_',
        };
        // Update 'lossy conversion' flag
        lossy_conv = lossy_conv || (fixed_c != c);
        // short name is always uppercase
        let upper = fixed_c.to_ascii_uppercase();
        dst[dst_pos] = upper as u8; // SAFE: upper is in range 0x20-0x7F
        dst_pos += 1;
    }
    (dst_pos, true, lossy_conv)
}

/// 由长文件名生成短文件名
pub fn generate_short_name(name: &str) -> [u8; SHORT_NAME_LEN] {
    // padded by ' '
    let mut short_name = [SHORT_FILE_NAME_PADDING; SHORT_NAME_LEN];
    // find extension after last dot
    // Note: short file name cannot start with the extension
    let dot_index_opt = name[1..].rfind('.').map(|index| index + 1);
    // copy basename (part of filename before a dot)
    let basename_src = dot_index_opt.map_or(name, |dot_index| &name[..dot_index]);
    let (_, basename_fits, basename_lossy) =
        copy_short_name_part(&mut short_name[0..8], basename_src);
    // copy file extension if exists
    dot_index_opt.map_or((basename_fits, basename_lossy), |dot_index| {
        let (_, ext_fits, ext_lossy) =
            copy_short_name_part(&mut short_name[8..11], &name[dot_index + 1..]);
        (basename_fits && ext_fits, basename_lossy || ext_lossy)
    });
    short_name
}

/// 对目录项的再一层抽象,可以理解对文件夹或文件的抽象
#[derive(Clone)]
pub struct VFile {
    name: String,
    short_cluster: usize,              // 文件短目录项所在扇区
    short_offset: usize,               // 文件短目录项所在偏移
    long_pos_vec: Vec<(usize, usize)>, // 长目录项的位置<cluster, offset>
    attribute: FileAttributes,
    fs: Arc<RwLock<RunFileSystem>>,
}

impl VFile {
    pub fn new(
        name: String,
        short_cluster: usize,
        short_offset: usize,
        long_pos_vec: Vec<(usize, usize)>,
        attribute: FileAttributes,
        fs: Arc<RwLock<RunFileSystem>>,
    ) -> Self {
        Self {
            name,
            short_cluster,
            short_offset,
            long_pos_vec,
            attribute,
            fs,
        }
    }
    pub fn is_root(&self) -> bool {
        self.name == "/"
    }
    pub fn name(&self) -> &str {
        self.name.as_str()
    }
    pub fn long_pos(&self) -> Vec<(usize, usize)> {
        self.long_pos_vec.clone()
    }
    pub fn short_pos(&self) -> (usize, usize) {
        (self.short_cluster, self.short_offset)
    }
    pub fn attribute(&self) -> FileAttributes {
        self.attribute
    }
    pub fn fs(&self) -> Arc<RwLock<RunFileSystem>> {
        self.fs.clone()
    }
    pub fn is_dir(&self) -> bool {
        self.attribute().contains(FileAttributes::DIRECTORY)
    }
    pub fn is_file(&self) -> bool {
        !self.is_dir()
    }
    pub fn clear_cache(&self) {
        // let fat = self.fs.read();
        // fat.cache_write_back();
    }
    pub fn first_data_cluster(&self) -> u32 {
        if self.is_root() {
            self.fs.read().bpb().root_dir_cluster()
        } else {
            self.fs.read().data_manager_modify().read_short_dirent(
                self.short_cluster,
                self.short_offset,
                |short_entry: &ShortDirectoryEntry| short_entry.first_cluster(),
            )
        }
    }
    pub fn last_data_cluster(&self) -> u32 {
        self.fs.read().data_manager_modify().read_short_dirent(
            self.short_cluster,
            self.short_offset,
            |short_entry: &ShortDirectoryEntry| short_entry.first_cluster(),
        )
    }
    pub fn set_first_cluster(&self, clu: u32) {
        self.fs.read().data_manager_modify().modify_short_dirent(
            self.short_cluster,
            self.short_offset,
            |se: &mut ShortDirectoryEntry| {
                se.set_first_cluster(clu);
            },
        )
    }
    pub fn read_at(&self, offset: usize, buf: &mut [u8]) -> usize {
        let mut entry = ShortDirectoryEntry::default();
        if self.is_root() {
            entry = self.fs.read().root_dirent();
        } else {
            self.fs.read().data_manager_modify().read_short_dirent(
                self.short_cluster,
                self.short_offset,
                |short_entry: &ShortDirectoryEntry| entry = *short_entry,
            );
        }
        entry.read_at(offset, buf, &self.fs)
    }
    pub fn write_at(&self, offset: usize, buf: &[u8]) -> usize {
        self.adjust_capacity(offset + buf.len()).unwrap();
        let mut entry = ShortDirectoryEntry::default();
        if self.is_root() {
            entry = self.fs.read().root_dirent();
        } else {
            self.fs.read().data_manager_modify().read_short_dirent(
                self.short_cluster,
                self.short_offset,
                |short_entry: &ShortDirectoryEntry| entry = *short_entry,
            );
        }
        let size = entry.write_at(offset, buf, &self.fs);
        if self.is_file() {
            self.fs.read().data_manager_modify().modify_short_dirent(
                self.short_cluster,
                self.short_offset,
                |short_entry: &mut ShortDirectoryEntry| short_entry.set_size(size as u32),
            );
        }
        size
    }
    /// 长文件名方式搜索, 只支持本级搜索, 不支持递归搜索
    fn find_long_name(&self, name: &str, dir_entry: &ShortDirectoryEntry) -> Option<VFile> {
        // 名字已经做了逆序处理
        // println!("name: {:#?}", name);
        let name_vec: Vec<[u16; LONG_NAME_LEN]> = long_name_split(name).into_iter().rev().collect();
        let entry_num = name_vec.len();
        let mut long_entry = LongDirectoryEntry::default();
        let mut long_pos_vec: Vec<(usize, usize)> = Vec::new();
        let mut dir_offset: usize = 0;
        let name_last = name_vec[0];
        // println!("name_last: {:#?}", name_last);
        loop {
            long_pos_vec.clear();
            // 读取 offset 处的目录项
            let mut read_sz = dir_entry.read_at(dir_offset, long_entry.as_bytes_mut(), &self.fs);
            // println!("here0");
            // 以下成立说明读到头了
            if read_sz != DIRENT_SZ {
                return None;
            }
            // println!("here1");
            if long_entry.is_free() {
                dir_offset += DIRENT_SZ;
                continue;
            }
            // println!("here2");
            let long_entry_name = long_entry.name_to_array();
            // println!("long_entry_name: {:#?}", long_entry_name);
            if long_entry_name == name_last && long_entry.is_long() {
                let raw_order = long_entry.raw_order();
                let long_checksum = long_entry.checksum();
                if !long_entry.is_last() || raw_order != entry_num as u8 {
                    dir_offset += DIRENT_SZ;
                    continue;
                }
                // 如果 order 也匹配，开一个循环继续匹配长名目录项
                let mut is_match = true;
                for i in 1..(raw_order as usize) {
                    read_sz = dir_entry.read_at(
                        dir_offset + i * DIRENT_SZ,
                        long_entry.as_bytes_mut(),
                        &self.fs,
                    );
                    if read_sz != DIRENT_SZ {
                        return None;
                    }
                    if long_entry.name_to_array() != name_vec[i] || !long_entry.is_long() {
                        is_match = false;
                        break;
                    }
                }
                if is_match {
                    // 如果成功，读短目录项，进行校验
                    let mut short_entry = ShortDirectoryEntry::default();
                    let short_offset = dir_offset + entry_num * DIRENT_SZ;
                    read_sz = dir_entry.read_at(short_offset, short_entry.as_bytes_mut(), &self.fs);
                    if read_sz != DIRENT_SZ || short_entry.is_free() {
                        return None;
                    }
                    let short_checksum = short_entry.checksum();
                    // println!("short_checksum: {:#X?}", short_checksum);
                    if long_checksum == short_checksum {
                        let (short_cluster, short_offset) = dir_entry.pos(short_offset, &self.fs);
                        for i in 0..raw_order as usize {
                            // 存入长名目录项位置了，第一个在栈顶
                            let (long_cluster, dir_offset) =
                                dir_entry.pos(dir_offset + i, &self.fs);
                            long_pos_vec.push((long_cluster.unwrap(), dir_offset));
                        }
                        return Some(VFile::new(
                            String::from(name),
                            short_cluster.unwrap(),
                            short_offset,
                            long_pos_vec,
                            short_entry.attribute(),
                            self.fs.clone(),
                        ));
                    } else {
                        return None; // QUES
                    }
                } else {
                    dir_offset += DIRENT_SZ;
                    continue;
                }
            } else {
                dir_offset += DIRENT_SZ;
            }
        }
    }
    /// 短文件名搜索, 只支持本级搜索, 不支持递归搜索
    fn find_short_name(&self, name: &str, dirent: &ShortDirectoryEntry) -> Option<VFile> {
        let mut short_entry = ShortDirectoryEntry::default();
        let mut offset = 0;
        let mut read_size;
        loop {
            read_size = dirent.read_at(offset, short_entry.as_bytes_mut(), &self.fs);
            if read_size != DIRENT_SZ || short_entry.is_free() {
                return None;
            } else {
                if (!short_entry.is_free()) && short_entry.is_short() && name == short_entry.name()
                {
                    let (short_cluster, short_offset) = dirent.pos(offset, &self.fs);
                    let long_pos_vec: Vec<(usize, usize)> = Vec::new();
                    return Some(VFile::new(
                        String::from(name),
                        short_cluster.unwrap_or(0),
                        short_offset,
                        long_pos_vec,
                        short_entry.attribute(),
                        self.fs.clone(),
                    ));
                } else {
                    offset += DIRENT_SZ;
                    continue;
                }
            }
        }
    }
    /// 根据名称搜索, 默认用长文件名搜索, 搜不到再用短文件名搜
    pub fn find_vfile_byname(&self, name: &str) -> Option<VFile> {
        assert!(self.is_dir());
        // 复制文件夹自己的短文件名目录项
        let mut short_entry = ShortDirectoryEntry::default();
        if self.is_root() {
            short_entry = self.fs.write().root_dirent();
        } else {
            self.fs.read().data_manager_modify().read_short_dirent(
                self.short_cluster,
                self.short_offset,
                |entry: &ShortDirectoryEntry| short_entry = *entry,
            );
        }
        // 长文件名搜索
        let res = self.find_long_name(name, &short_entry);
        if res.is_some() {
            return res;
        } else {
            // 如果是 .. 则只能用短文件名搜索
            return self.find_short_name(name, &short_entry);
        }
    }
    /// 根据路径递归搜索, 需要区分是绝对路径还是相对路径
    pub fn find_vfile_bypath(&self, path: &str) -> Option<Arc<VFile>> {
        let pathv: Vec<&str> = path.split('/').collect();
        let len = pathv.len();
        if pathv.len() == 0 {
            return None;
        }
        // 如果第一个串为空, 说明是绝对路径
        if pathv[0] == "" {
            // println!("here0");
            let mut current_vfile = self.fs.read().root_vfile(&self.fs);
            for i in 1..len {
                // println!("here1");
                if pathv[i] == "" || pathv[i] == "." {
                    continue;
                }
                if let Some(vfile) = current_vfile.find_vfile_byname(pathv[i]) {
                    current_vfile = vfile;
                } else {
                    return None;
                }
            }
            // println!("here100");
            Some(Arc::new(current_vfile))
        }
        // 如果第一个串不为空, 说明是相对路径
        else {
            let mut current_vfile = self.clone();
            for i in 0..len {
                if pathv[i] == "" || pathv[i] == "." {
                    continue;
                }
                if let Some(vfile) = current_vfile.find_vfile_byname(pathv[i]) {
                    current_vfile = vfile;
                } else {
                    return None;
                }
            }
            Some(Arc::new(current_vfile))
        }
    }
    /// 计算文件或文件夹大小, 如果是文件夹, 大小就是全部目录项的总字节数, 如果是文件, 大小就是内容的字节数
    pub fn size(&self) -> usize {
        if self.is_dir() {
            let mut offset = 0;
            let mut size = 0;
            let mut tmp_dirent = ShortDirectoryEntry::default();
            loop {
                let read_size = self.read_at(offset, tmp_dirent.as_bytes_mut());
                if read_size == 0 {
                    return size;
                }
                if !tmp_dirent.is_free() {
                    size += DIRENT_SZ;
                }
                offset += DIRENT_SZ;
            }
        } else {
            self.fs.read().data_manager_modify().read_short_dirent(
                self.short_cluster,
                self.short_offset,
                |short_entry: &ShortDirectoryEntry| short_entry.size().unwrap(),
            ) as usize
        }
    }
    /// 计算文件或文件夹容量, 容量就是全部簇的总字节数
    pub fn capacity(&self) -> usize {
        let cluster_size = self.fs.read().bpb().cluster_size();
        cluster_size
            * self
                .fs
                .read()
                .fat_manager_modify()
                .count_clusters(self.first_data_cluster() as usize)
    }
    /// 改变文件或文件夹的容量, 成功返回 Ok, 失败返回 GG
    /// new_capacity 不一定要是 cluster_size 的整数倍, 函数会帮忙向上取整
    // TODO: 减少容量
    pub fn adjust_capacity(&self, new_capacity: usize) -> Result<(), FSError> {
        let cluster_size = self.fs.read().bpb().cluster_size();
        let current_capacity = self.capacity();
        // println!("current_capacity: {}", current_capacity);
        let num = (new_capacity + cluster_size - current_capacity) / cluster_size;
        // println!("num: {}", num);
        let current_last_cluster = self
            .fs
            .read()
            .fat_manager_modify()
            .last_cluster(self.first_data_cluster() as usize);
        self.fs
            .write()
            .alloc_clusters(num, Some(current_last_cluster as u32));
        Ok(())
    }
    /// 查找可用目录项，返回 offset，簇不够会增加
    pub fn find_free_dirents(&self, num: usize) -> Option<usize> {
        // println!("0-0-1-0");
        if self.is_file() || num == 0 {
            // println!("0-0-1-1");
            return None;
        }
        let mut offset = 0;
        loop {
            // println!("0-0-1-2");
            let mut tmp_dirent = ShortDirectoryEntry::default();
            let mut read_size = self.read_at(offset, tmp_dirent.as_bytes_mut());
            // println!("0-0-1-3");
            // 扩容
            if read_size == 0 {
                let current_capacity = self.capacity();
                // println!("new_capacity: {}", current_capacity + num * DIRENT_SZ);
                self.adjust_capacity(current_capacity + num * DIRENT_SZ)
                    .unwrap();
            }
            // 找到第一个空簇
            if tmp_dirent.is_free() {
                let first_offset = offset;
                let mut available = 1;
                while available < num {
                    offset += DIRENT_SZ;
                    read_size = self.read_at(offset, tmp_dirent.as_bytes_mut());
                    let current_capacity = self.capacity();
                    // 不够了,扩容后再读
                    if read_size == 0 && offset >= current_capacity {
                        // println!("new_capacity: {}", current_capacity + num * DIRENT_SZ);
                        self.adjust_capacity(current_capacity + num * DIRENT_SZ)
                            .unwrap();
                        self.read_at(offset, tmp_dirent.as_bytes_mut());
                    }
                    if tmp_dirent.is_free() {
                        available += 1;
                    } else {
                        break;
                    }
                }
                // 找到足够的空间安排目录项
                if available == num {
                    // println!("0-0-1-4");
                    // println!("offset: {:#?} {:#X?}", offset, offset);
                    return Some(first_offset);
                }
            }
            // println!("0-0-1-5");
            offset += DIRENT_SZ;
        }
    }
    pub fn is_already_exist(&self, filename: &str, attribute: FileAttributes) -> bool {
        if let Some(vfile) = self.find_vfile_byname(filename) {
            return (vfile.is_dir() && attribute.contains(FileAttributes::DIRECTORY))
                || (!vfile.is_dir() && !attribute.contains(FileAttributes::DIRECTORY));
        }
        false
    }
    /// 在当前目录下创建文件或目录
    pub fn create(&self, filename: &str, attribute: FileAttributes) -> Option<Arc<VFile>> {
        // 判断是否是文件夹
        assert!(self.is_dir());
        /* 如果已经存在了就返回 None */
        if self.is_already_exist(filename, attribute) {
            return None;
        }
        /* 不存在就创建文件 */
        // 长文件名拆分
        let mut long_name_vec = long_name_split(filename);
        let long_entry_num = long_name_vec.len();
        // println!("need_dirent: {}", long_entry_num + 1);
        // 搜索能够在文件夹里创建足够目录项的空处
        let mut dirent_offset: usize;
        if let Some(offset) = self.find_free_dirents(long_entry_num + 1) {
            dirent_offset = offset;
        } else {
            return None;
        }
        // println!("dirent_offset: {}", dirent_offset);
        // 生成短文件名及对应目录项
        let short_name: [u8; SHORT_NAME_LEN] = generate_short_name(filename);
        let mut name = [0u8; SHORT_FILE_NAME_LEN];
        name.copy_from_slice(&short_name[0..SHORT_FILE_NAME_LEN]);
        let mut ext = [0u8; SHORT_FILE_EXT_LEN];
        ext.copy_from_slice(&short_name[SHORT_FILE_NAME_LEN..SHORT_NAME_LEN]);
        // 给文件或文件夹分配空间
        let first_data_cluster = self.fs.write().alloc_cluster(None).unwrap();
        // println!("first_data_cluster: {}", first_data_cluster);
        let short_entry = ShortDirectoryEntry::new(name, ext, attribute, first_data_cluster);
        let checksum = short_entry.checksum();
        // println!("long_entry_num: {}", long_entry_num);
        // 写长目录项
        for i in 0..long_entry_num {
            let mut order: u8 = (long_entry_num - i) as u8;
            if i == 0 {
                order |= LAST_LONG_ENTRY;
            }
            let long_entry = LongDirectoryEntry::new(long_name_vec.pop().unwrap(), order, checksum);
            assert_eq!(
                self.write_at(dirent_offset, long_entry.as_bytes()),
                DIRENT_SZ
            );
            dirent_offset += DIRENT_SZ;
            // println!("dirent_offset: {}", dirent_offset);
        }
        // 写短目录项
        assert_eq!(
            self.write_at(dirent_offset, short_entry.as_bytes()),
            DIRENT_SZ
        );
        // 检查文件是否创建成功
        let vfile = self.find_vfile_byname(filename).unwrap();
        // 如果是目录类型，需要创建 .和 ..(根目录不需要, 但显然不会去创建根目录)
        if attribute.contains(FileAttributes::DIRECTORY) {
            let dot: [u8; SHORT_NAME_LEN] = generate_short_name(".");
            name.copy_from_slice(&dot[0..SHORT_FILE_NAME_LEN]);
            ext.copy_from_slice(&dot[SHORT_FILE_NAME_LEN..SHORT_NAME_LEN]);
            let self_dir =
                ShortDirectoryEntry::new(name, ext, FileAttributes::DIRECTORY, first_data_cluster);
            let dotdot: [u8; SHORT_NAME_LEN] = generate_short_name("..");
            name.copy_from_slice(&dotdot[0..SHORT_FILE_NAME_LEN]);
            ext.copy_from_slice(&dotdot[SHORT_FILE_NAME_LEN..SHORT_NAME_LEN]);
            let parent_dir;
            if self.is_root() {
                parent_dir = ShortDirectoryEntry::new(name, ext, FileAttributes::DIRECTORY, 0);
            } else {
                parent_dir = ShortDirectoryEntry::new(
                    name,
                    ext,
                    FileAttributes::DIRECTORY,
                    self.first_data_cluster() as u32,
                );
            }
            vfile.write_at(0, self_dir.as_bytes());
            vfile.write_at(DIRENT_SZ, parent_dir.as_bytes());
        }
        return Some(Arc::new(vfile));
    }
    // 清空文件
    // pub fn clear(&self) {
    //     // 难点:长名目录项也要修改
    //     let first_cluster: u32 = self.first_cluster();
    //     if self.is_dir() || first_cluster == 0 {
    //         return;
    //     }
    //     for i in 0..self.long_pos_vec.len() {
    //         self.modify_long_dirent(i, |long_ent: &mut LongDirectoryEntry| {
    //             long_ent.clear();
    //         });
    //     }
    //     self.modify_short_dirent(|short_ent: &mut ShortDirectoryEntry| {
    //         short_ent.clear();
    //     });
    //     let all_clusters = self
    //         .fs
    //         .read()
    //         .get_fat()
    //         .read()
    //         .get_all_cluster_of(first_cluster, self.block_device.clone());
    //     //self.fs.write().dealloc_cluster(all_clusters);
    //     let fs_reader = self.fs.read();
    //     fs_reader.dealloc_cluster(all_clusters);
    //     //fs_reader.cache_write_back();
    // }

    /// 目前只支持删除文件自己, 不能递归删除, 也无法清空文件夹, 如果文件夹里有东西, 那就等着悬空吧
    pub fn delete(&self) -> usize {
        // println!(
        //     "entry cluster_id: {}, offset: {}",
        //     self.short_cluster, self.short_offset
        // );
        let first_cluster: u32 = self.first_data_cluster();
        // println!("file first_cluster: {}", first_cluster);
        for (cluster, offset) in self.long_pos_vec.iter() {
            // println!("cluster_id: {}, offset: {}", *cluster, *offset);
            self.fs.read().data_manager_modify().modify_long_dirent(
                *cluster,
                *offset,
                |long_entry: &mut LongDirectoryEntry| {
                    long_entry.set_deleted();
                },
            );
        }
        self.fs.read().data_manager_modify().modify_short_dirent(
            self.short_cluster,
            self.short_offset,
            |short_entry: &mut ShortDirectoryEntry| {
                short_entry.set_deleted();
            },
        );
        self.fs
            .write()
            .dealloc_clusters(first_cluster as usize, None)
    }
    /// 获取目录中 offset 处目录项的信息
    /// 如果 offset 处内容为空, 则返回空, 成功返回<name, offset, first_cluster, attributes>
    pub fn dirent_info(&self, mut offset: usize) -> Option<(String, usize, u32, FileAttributes)> {
        if !self.is_dir() {
            return None;
        }
        let mut long_entry = LongDirectoryEntry::default();
        let mut name = String::new();
        // 读第一个长文件名目录项
        let mut read_size = self.read_at(offset, long_entry.as_bytes_mut());
        if read_size != DIRENT_SZ
            || !long_entry.is_long()
            || long_entry.is_free()
            || !long_entry.is_last()
        {
            return None;
        }
        // 确认了第一长文件名目录项后读剩余的文件名
        let raw_order = long_entry.raw_order();
        name.insert_str(0, long_entry.name_format().as_str());
        for _ in 1..raw_order {
            offset += DIRENT_SZ;
            read_size = self.read_at(offset, long_entry.as_bytes_mut());
            if read_size != DIRENT_SZ || !long_entry.is_long() || long_entry.is_free() {
                return None;
            }
            name.insert_str(0, long_entry.name_format().as_str());
        }
        // 读短文件名目录项
        let mut short_entry = ShortDirectoryEntry::default();
        offset += DIRENT_SZ;
        read_size = self.read_at(offset, short_entry.as_bytes_mut());
        if read_size != DIRENT_SZ || !short_entry.is_short() || short_entry.is_free() {
            return None;
        }
        let attribute = short_entry.attribute();
        let first_cluster = short_entry.first_cluster();
        return Some((name, offset, first_cluster, attribute));
    }
    /// 获取目录中offset处目录项的信息 TODO:之后考虑和stat复用
    /// 返回<size, atime, mtime, ctime>
    pub fn stat(&self) -> (i64, i64, i64, i64, u64) {
        let mut stat = self.fs.read().data_manager_modify().read_short_dirent(
            self.short_cluster,
            self.short_offset,
            |short_entry: &ShortDirectoryEntry| {
                let (_, _, _, _, _, _, ctime) = short_entry.get_creation_time();
                let (_, _, _, _, _, _, atime) = short_entry.accessed_time();
                let (_, _, _, _, _, _, mtime) = short_entry.modification_time();
                let first_clu = short_entry.first_cluster();
                (
                    0i64,
                    atime as i64,
                    mtime as i64,
                    ctime as i64,
                    first_clu as u64,
                )
            },
        );
        stat.0 = self.size() as i64;
        stat
    }
    pub fn ls(&self) -> Option<Vec<(String, FileAttributes)>> {
        if self.is_file() {
            return None;
        }
        let mut list: Vec<(String, FileAttributes)> = Vec::new();
        let capacity = self.capacity();
        let mut offset = 0;
        while offset < capacity {
            let item = self.dirent_info(offset);
            match item {
                None => offset += DIRENT_SZ,
                Some(s) => {
                    list.push((s.0, s.3));
                    offset = s.1 + DIRENT_SZ
                }
            }
        }
        return Some(list);
    }
}

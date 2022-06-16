/// 虚拟文件系统, 将实际文件系统抽象成满足文件,文件夹创建读写删除功能的抽象文件系统
use super::{
    FileAttributes, LongDirectoryEntry, RunFileSystem, ShortDirectoryEntry, DIRENT_SZ,
    LAST_LONG_ENTRY, LONG_NAME_LEN, SHORT_FILE_EXT_LEN, SHORT_FILE_NAME_LEN,
    SHORT_FILE_NAME_PADDING, SHORT_NAME_LEN,
};
use spin::RwLock;
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
    let (basename_len, basename_fits, basename_lossy) =
        copy_short_name_part(&mut short_name[0..8], basename_src);
    // copy file extension if exists
    let (name_fits, lossy_conv) =
        dot_index_opt.map_or((basename_fits, basename_lossy), |dot_index| {
            let (_, ext_fits, ext_lossy) =
                copy_short_name_part(&mut short_name[8..11], &name[dot_index + 1..]);
            (basename_fits && ext_fits, basename_lossy || ext_lossy)
        });
    short_name
}

/// 拆分文件名和后缀
pub fn split_name_ext<'a>(name: &'a str) -> (&'a str, &'a str) {
    let mut name_and_ext: Vec<&str> = name.split(".").collect();
    let name = name_and_ext[0];
    if name_and_ext.len() == 1 {
        name_and_ext.push("");
    }
    let ext = name_and_ext[1];
    (name, ext)
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
    pub fn first_cluster(&self) -> u32 {
        self.fs.read().data_manager_modify().read_short_dirent(
            self.short_cluster,
            self.short_offset,
            |se: &ShortDirectoryEntry| se.first_cluster(),
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
        self.fs.read().data_manager_modify().read_short_dirent(
            self.short_cluster,
            self.short_offset,
            |short_ent: &ShortDirectoryEntry| short_ent.read_at(offset, buf, &self.fs),
        )
    }
    pub fn write_at(&self, offset: usize, buf: &[u8]) -> usize {
        // self.increase_size((offset + buf.len()) as u32);
        self.fs.read().data_manager_modify().modify_short_dirent(
            self.short_cluster,
            self.short_offset,
            |short_ent: &mut ShortDirectoryEntry| short_ent.write_at(offset, buf, &self.fs),
        )
    }
    /// 长文件名方式搜索, 只支持本级搜索, 不支持递归搜索
    fn find_long_name(&self, name: &str, dir_entry: &ShortDirectoryEntry) -> Option<VFile> {
        // 名字已经做了逆序处理
        let name_vec: Vec<[u16; 13]> = long_name_split(name).into_iter().rev().collect();
        // println!("name_vec: {:#?}", name_vec);
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
            // 以下成立说明读到头了
            if read_sz != DIRENT_SZ || long_entry.is_free() {
                return None;
            }
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
        let name_upper = name.to_uppercase();
        let mut short_entry = ShortDirectoryEntry::default();
        let mut offset = 0;
        let mut read_size;
        loop {
            read_size = dirent.read_at(offset, short_entry.as_bytes_mut(), &self.fs);
            if read_size != DIRENT_SZ || short_entry.is_free() {
                return None;
            } else {
                if (!short_entry.is_free())
                    && short_entry.is_short()
                    && name_upper == short_entry.name()
                {
                    let (short_cluster, short_offset) = dirent.pos(offset, &self.fs);
                    let long_pos_vec: Vec<(usize, usize)> = Vec::new();
                    return Some(VFile::new(
                        String::from(name_upper),
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
        let mut split_name: Vec<&str> = name.split(".").collect();
        if split_name.len() == 1 {
            split_name.push("");
        }
        let mut short_entry = ShortDirectoryEntry::default();
        self.fs.read().data_manager_modify().read_short_dirent(
            self.short_cluster,
            self.short_offset,
            |entry: &ShortDirectoryEntry| short_entry = *entry,
        );
        // 长文件名搜索
        let res = self.find_long_name(name, &short_entry);
        if res.is_some() {
            // println!("long");
            return res;
        } else {
            println!("short");
            // 短文件名
            return self.find_short_name(&name, &short_entry);
        }
    }
    /// 根据路径递归搜索, 需要区分是绝对路径还是相对路径
    pub fn find_vfile_bypath(&self, path: Vec<&str>) -> Option<Arc<VFile>> {
        let _ = self.fs.read(); // 获取读锁
        let len = path.len();
        if len == 0 {
            return None;
        }
        let mut current_vfile = self.clone();
        for i in 0..len {
            if path[i] == "" || path[i] == "." {
                continue;
            }
            if let Some(vfile) = current_vfile.find_vfile_byname(path[i]) {
                current_vfile = vfile;
            } else {
                return None;
            }
        }
        Some(Arc::new(current_vfile))
    }
    /// 查找可用目录项，返回 offset，簇不够也会返回相应的 offset，caller 需要及时分配
    fn find_free_dirent(&self) -> Option<usize> {
        if self.is_file() {
            return None;
        }
        let mut offset = 0;
        loop {
            let mut tmp_dirent = ShortDirectoryEntry::default();
            let read_sz = self.fs.read().data_manager_modify().read_short_dirent(
                self.short_cluster,
                self.short_offset,
                |short_ent: &ShortDirectoryEntry| {
                    short_ent.read_at(offset, tmp_dirent.as_bytes_mut(), &self.fs)
                },
            );
            if tmp_dirent.is_free() || read_sz == 0 {
                return Some(offset);
            }
            offset += DIRENT_SZ;
        }
    }
    /// 在当前目录下创建文件或目录
    pub fn create(&self, filename: &str, attribute: FileAttributes) -> Option<Arc<VFile>> {
        // 检测同名文件
        assert!(self.is_dir());
        // 搜索空处
        let mut dirent_offset: usize;
        if let Some(offset) = self.find_free_dirent() {
            dirent_offset = offset;
        } else {
            return None;
        }
        // 长文件名拆分
        let mut long_name_vec = long_name_split(filename);
        let long_entry_num = long_name_vec.len();
        // 生成短文件名及对应目录项
        let short_name: [u8; SHORT_NAME_LEN] = generate_short_name(filename);
        let mut name = [0u8; SHORT_FILE_NAME_LEN];
        name.copy_from_slice(&short_name[0..SHORT_FILE_NAME_LEN]);
        let mut ext = [0u8; SHORT_FILE_EXT_LEN];
        ext.copy_from_slice(&short_name[SHORT_FILE_NAME_LEN..SHORT_NAME_LEN]);
        let short_entry = ShortDirectoryEntry::new(name, ext, attribute);
        let checksum = short_entry.checksum();
        // 写长名目录项
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
        }
        // 写短目录项
        assert_eq!(
            self.write_at(dirent_offset, short_entry.as_bytes()),
            DIRENT_SZ
        );
        if let Some(vfile) = self.find_vfile_byname(filename) {
            // 如果是目录类型，需要创建.和..
            if attribute.contains(FileAttributes::DIRECTORY) {
                let dot: [u8; SHORT_NAME_LEN] = generate_short_name(".");
                let mut name = [0u8; SHORT_FILE_NAME_LEN];
                name.copy_from_slice(&dot[0..SHORT_FILE_NAME_LEN]);
                let mut ext = [0u8; SHORT_FILE_EXT_LEN];
                ext.copy_from_slice(&dot[SHORT_FILE_NAME_LEN..SHORT_NAME_LEN]);
                let mut self_dir = ShortDirectoryEntry::new(name, ext, FileAttributes::DIRECTORY);
                let dotdot: [u8; SHORT_NAME_LEN] = generate_short_name("..");
                let mut name = [0u8; SHORT_FILE_NAME_LEN];
                name.copy_from_slice(&dotdot[0..SHORT_FILE_NAME_LEN]);
                let mut ext = [0u8; SHORT_FILE_EXT_LEN];
                ext.copy_from_slice(&dotdot[SHORT_FILE_NAME_LEN..SHORT_NAME_LEN]);
                let mut parent_dir = ShortDirectoryEntry::new(name, ext, FileAttributes::DIRECTORY);
                parent_dir.set_first_cluster(self.first_cluster());
                vfile.write_at(0, self_dir.as_bytes_mut());
                vfile.write_at(DIRENT_SZ, parent_dir.as_bytes_mut());
                let first_cluster = self.fs.read().data_manager_modify().read_short_dirent(
                    self.short_cluster,
                    self.short_offset,
                    |se: &ShortDirectoryEntry| se.first_cluster(),
                );
                self_dir.set_first_cluster(first_cluster);
                vfile.write_at(0, self_dir.as_bytes_mut());
            }
            return Some(Arc::new(vfile));
        } else {
            None
        }
    }
    /// 目前只支持删除文件自己, 不能递归删除, 也无法清空文件夹
    pub fn delete(&self) -> usize {
        let first_cluster: u32 = self.first_cluster();
        for (cluster, offset) in self.long_pos_vec.iter() {
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
        let all_clusters = self
            .fs
            .read()
            .fat_manager_modify()
            .all_clusters(first_cluster as usize);
        self.fs.write().dealloc_clusters(all_clusters[0], None);
        return all_clusters.len();
    }
}

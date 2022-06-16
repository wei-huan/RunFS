/// 虚拟文件系统, 将实际文件系统抽象成满足文件,文件夹创建读写删除功能的抽象文件系统
use super::{
    FileAttributes, LongDirectoryEntry, RunFileSystem, ShortDirectoryEntry, DIRENT_SZ,
    LONG_NAME_LEN,
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
    // pub fn read_short_dirent<V>(&self, f: impl FnOnce(&ShortDirectoryEntry) -> V) -> V {
    //     let runfs = self.fs.read();
    //     if self.short_cluster == runfs.bpb().root_dir_cluster() as usize {
    //         let root_dirent = runfs.root_dirent();
    //         let rr = root_dirent.read();
    //         f(&rr)
    //     } else {
    //         runfs
    //             .data_manager_modify()
    //             .read_cluster_at(self.short_cluster, self.short_offset, f)
    //     }
    // }
    // pub fn modify_short_dirent<V>(&self, f: impl FnOnce(&mut ShortDirectoryEntry) -> V) -> V {
    //     let runfs = self.fs.read();
    //     if self.short_cluster == runfs.bpb().root_dir_cluster() as usize {
    //         let root_dirent = runfs.root_dirent();
    //         let mut rw = root_dirent.write();
    //         f(&mut rw)
    //     } else {
    //         runfs
    //             .data_manager_modify()
    //             .write_cluster_at(self.short_cluster, self.short_offset, f)
    //     }
    // }
    // fn modify_long_dirent<V>(
    //     &self,
    //     index: usize,
    //     f: impl FnOnce(&mut LongDirectoryEntry) -> V,
    // ) -> V {
    //     let runfs = self.fs.read();
    //     let (cluster, offset) = self.long_pos_vec[index];
    //     runfs
    //         .data_manager_modify()
    //         .write_cluster_at(cluster, offset, f)
    // }
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
        let entry_num = name_vec.len();
        let mut long_entry = LongDirectoryEntry::default();
        let mut long_pos_vec: Vec<(usize, usize)> = Vec::new();
        let mut offset: usize = 0;
        let name_last = name_vec[0];
        loop {
            long_pos_vec.clear();
            // 读取 offset 处的目录项
            let mut read_sz = dir_entry.read_at(offset, long_entry.as_bytes_mut(), &self.fs);
            if read_sz != DIRENT_SZ || long_entry.is_empty() {
                return None;
            }
            if long_entry.name_to_array() == name_last && long_entry.is_long() {
                let order = long_entry.order();
                let raw_order = long_entry.raw_order();
                let long_checksum = long_entry.checksum();
                if !long_entry.is_last()
                    || long_entry.is_free()
                    || raw_order != entry_num as u8
                {
                    offset += DIRENT_SZ;
                    continue;
                }
                // 如果 order 也匹配，开一个循环继续匹配长名目录项
                let mut is_match = true;
                for i in 1..(raw_order as usize) {
                    read_sz = dir_entry.read_at(
                        offset + i * DIRENT_SZ,
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
                    let short_offset = offset + entry_num * DIRENT_SZ;
                    read_sz = dir_entry.read_at(short_offset, short_entry.as_bytes_mut(), &self.fs);
                    if read_sz != DIRENT_SZ {
                        return None;
                    }
                    if !short_entry.is_free() && long_checksum == short_entry.checksum() {
                        let (short_cluster, short_offset) = dir_entry.pos(short_offset, &self.fs);
                        for i in 0..order as usize {
                            // 存入长名目录项位置了，第一个在栈顶
                            let (long_cluster, offset) = dir_entry.pos(offset + i, &self.fs);
                            long_pos_vec.push((long_cluster.unwrap(), offset));
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
                    offset += DIRENT_SZ;
                    continue;
                }
            } else {
                offset += DIRENT_SZ;
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

    // 根据名称搜索, 默认用长文件名搜索, 搜不到再用短文件名搜
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
        // 长文件名
        let res = self.find_long_name(name, &short_entry);
        if res.is_some() {
            return res;
        } else {
            // 短文件名
            return self.find_short_name(&name, &short_entry);
        }
    }

    /// 根据路径递归搜索, 需要区分是绝对路径还是相对路径
    // pub fn find_vfile_bypath(&self, path: Vec<&str>) -> Option<Arc<VFile>> {
    //     let _ = self.fs.read(); // 获取读锁
    //     let len = path.len();
    //     if len == 0 {
    //         return None;
    //     }
    //     let mut current_vfile = self.clone();
    //     for i in 0..len {
    //         if path[i] == "" || path[i] == "." {
    //             continue;
    //         }
    //         if let Some(vfile) = current_vfile.find_vfile_byname(path[i]) {
    //             current_vfile = vfile;
    //         } else {
    //             return None;
    //         }
    //     }
    //     Some(Arc::new(current_vfile))
    // }

    /// 在当前目录下创建文件
    // pub fn create(&self, name: &str, attribute: FileAttributes) -> Option<Arc<VFile>> {
    //     // 检测同名文件
    //     assert!(self.is_dir());
    //     let runfs = self.fs.read();
    //     let (name_, ext_) = runfs.split_name_ext(name);
    //     // 搜索空处
    //     let mut dirent_offset: usize;
    //     if let Some(offset) = self.find_free_dirent() {
    //         dirent_offset = offset;
    //     } else {
    //         return None;
    //     }
    //     let mut short_ent = ShortDirectoryEntry::default();
    //     if name_.len() > 8 || ext_.len() > 3 {
    //         // 长文件名拆分
    //         let mut v_long_name = runfs.long_name_split(name);
    //         let long_ent_num = v_long_name.len();
    //         let mut long_ent = LongDirectoryEntry::default();
    //         // 生成短文件名及对应目录项
    //         let short_name = runfs.generate_short_name(name);
    //         let (name_bytes, ext_bytes) = runfs.short_name_format(short_name.as_str());
    //         short_ent.initialize(&name_bytes, &ext_bytes, attribute);
    //         let check_sum = short_ent.checksum();
    //         //println!("*** aft checksum");
    //         drop(runfs);
    //         // 写长名目录项
    //         for i in 0..long_ent_num {
    //             let mut order: u8 = (long_ent_num - i) as u8;
    //             if i == 0 {
    //                 order |= 0x40;
    //             }
    //             long_ent.initialize(v_long_name.pop().unwrap().as_bytes(), order, check_sum);
    //             assert_eq!(
    //                 self.write_at(dirent_offset, long_ent.as_bytes_mut()),
    //                 DIRENT_SZ
    //             );
    //             dirent_offset += DIRENT_SZ;
    //         }
    //     } else {
    //         // 短文件名格式化
    //         let (name_bytes, ext_bytes) = manager_reader.short_name_format(name);
    //         short_ent.initialize(&name_bytes, &ext_bytes, attribute);
    //         short_ent.set_case(ALL_LOWER_CASE);
    //         drop(manager_reader);
    //     }
    //     // 写短目录项
    //     assert_eq!(
    //         self.write_at(dirent_offset, short_ent.as_bytes_mut()),
    //         DIRENT_SZ
    //     );

    //     // 如果是目录类型，需要创建.和..
    //     if let Some(vfile) = self.find_vfile_byname(name) {
    //         if attribute & ATTRIBUTE_DIRECTORY != 0 {
    //             let manager_reader = self.fs.read();
    //             let (name_bytes, ext_bytes) = manager_reader.short_name_format(".");
    //             let mut self_dir =
    //                 ShortDirectoryEntry::new(&name_bytes, &ext_bytes, ATTRIBUTE_DIRECTORY);
    //             let (name_bytes, ext_bytes) = manager_reader.short_name_format("..");
    //             let mut par_dir =
    //                 ShortDirectoryEntry::new(&name_bytes, &ext_bytes, ATTRIBUTE_DIRECTORY);
    //             drop(manager_reader);
    //             par_dir.set_first_cluster(self.first_cluster());

    //             vfile.write_at(0, self_dir.as_bytes_mut());
    //             vfile.write_at(DIRENT_SZ, par_dir.as_bytes_mut());
    //             let first_cluster =
    //                 vfile.read_short_dirent(|se: &ShortDirectoryEntry| se.first_cluster());
    //             self_dir.set_first_cluster(first_cluster);
    //             vfile.write_at(0, self_dir.as_bytes_mut());
    //         }
    //         return Some(Arc::new(vfile));
    //     } else {
    //         None
    //     }
    // }

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

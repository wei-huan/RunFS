/// 虚拟文件系统, 将实际文件系统抽象成满足文件,文件夹创建读写删除功能的抽象文件系统
use super::{FileAttributes, RunFileSystem, ShortDirectoryEntry, DIRENT_SZ};
use spin::RwLock;
use std::sync::Arc;

/// 对目录项的再一层抽象,可以理解对文件夹或文件的抽象
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

    // /* 在当前目录下创建文件 */
    // pub fn create(&self, name: &str, attribute: u8) -> Option<Arc<VFile>> {
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

    // /* WAITING 目前只支持删除自己*/
    // pub fn remove(&self) -> usize {
    //     let first_cluster: u32 = self.first_cluster();
    //     for i in 0..self.long_pos_vec.len() {
    //         self.modify_long_dirent(i, |long_ent: &mut LongDirectoryEntry| {
    //             long_ent.delete();
    //         });
    //     }
    //     self.modify_short_dirent(|short_ent: &mut ShortDirectoryEntry| {
    //         short_ent.delete();
    //     });
    //     let all_clusters = self
    //         .fs
    //         .read()
    //         .get_fat()
    //         .read()
    //         .get_all_cluster_of(first_cluster, self.block_device.clone());
    //     self.fs.write().dealloc_cluster(all_clusters.clone());
    //     return all_clusters.len();
    // }

    // fn find_long_name(&self, name: &str, dir_ent: &ShortDirectoryEntry) -> Option<VFile> {
    //     let name_vec = self.fs.read().long_name_split(name);
    //     let mut offset: usize = 0;
    //     let mut long_ent = LongDirectoryEntry::empty();
    //     let long_ent_num = name_vec.len();
    //     let mut long_pos_vec: Vec<(usize, usize)> = Vec::new();
    //     let name_last = name_vec[long_ent_num - 1].clone();
    //     let mut step: usize = long_ent_num;
    //     for i in (long_ent_num - 2)..0 {
    //         if name_last == name_vec[i] {
    //             // step = step - i - 1;
    //             break;
    //         }
    //     }
    //     step = 1;
    //     loop {
    //         long_pos_vec.clear();
    //         // 读取offset处的目录项
    //         let mut read_sz = dir_ent.read_at(
    //             offset,
    //             long_ent.as_bytes_mut(),
    //             &self.fs,
    //             &self.fs.read().get_fat(),
    //             &self.block_device,
    //         );
    //         if read_sz != DIRENT_SZ || long_ent.is_empty() {
    //             return None;
    //         }
    //         if long_ent.get_name_raw() == name_last && long_ent.attribute() == ATTRIBUTE_LFN {
    //             // 匹配：如果名一致，且第一字段为0x4*，获取该order，以及校验和
    //             let mut order = long_ent.get_order();
    //             let l_checksum = long_ent.get_checksum();
    //             if order & 0x40 == 0 || order == 0xE5 {
    //                 offset += step * DIRENT_SZ;
    //                 continue;
    //             }
    //             order = order ^ 0x40;
    //             if order as usize != long_ent_num {
    //                 offset += step * DIRENT_SZ;
    //                 continue;
    //             }
    //             // 如果order也匹配，开一个循环继续匹配长名目录项
    //             let mut is_match = true;
    //             for i in 1..order as usize {
    //                 read_sz = dir_ent.read_at(
    //                     offset + i * DIRENT_SZ,
    //                     long_ent.as_bytes_mut(),
    //                     &self.fs,
    //                     &self.fs.read().get_fat(),
    //                     &self.block_device,
    //                 );
    //                 if read_sz != DIRENT_SZ {
    //                     return None;
    //                 }
    //                 if long_ent.get_name_raw() != name_vec[long_ent_num - 1 - i]
    //                     || long_ent.attribute() != ATTRIBUTE_LFN
    //                 {
    //                     is_match = false;
    //                     break;
    //                 }
    //             }
    //             if is_match {
    //                 // 如果成功，读短目录项，进行校验
    //                 let mut short_ent = ShortDirEntry::empty();
    //                 let s_off = offset + long_ent_num * DIRENT_SZ;
    //                 read_sz = dir_ent.read_at(
    //                     s_off,
    //                     short_ent.as_bytes_mut(),
    //                     &self.fs,
    //                     &self.fs.read().get_fat(),
    //                     &self.block_device,
    //                 );
    //                 if read_sz != DIRENT_SZ {
    //                     return None;
    //                 }
    //                 if short_ent.is_valid() && l_checksum == short_ent.checksum() {
    //                     let (short_sector, short_offset) = self.get_pos(s_off);
    //                     for i in 0..order as usize {
    //                         // 存入长名目录项位置了，第一个在栈顶
    //                         let pos = self.get_pos(offset + i);
    //                         long_pos_vec.push(pos);
    //                     }
    //                     return Some(VFile::new(
    //                         String::from(name),
    //                         short_sector,
    //                         short_offset,
    //                         long_pos_vec,
    //                         //short_ent.first_cluster(),
    //                         short_ent.attribute(),
    //                         short_ent.get_size(),
    //                         self.fs.clone(),
    //                         self.block_device.clone(),
    //                     ));
    //                 } else {
    //                     return None; // QUES
    //                 }
    //             } else {
    //                 offset += step * DIRENT_SZ;
    //                 continue;
    //             }
    //         } else {
    //             offset += step * DIRENT_SZ;
    //         }
    //     }
    // }
    /// 传进来的必须是大写纯 ASCII, 否则就按照长文件名方式读取
    fn find_short_name(&self, name_upper: &str, dirent: &ShortDirectoryEntry) -> Option<VFile> {
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
                    // println!("find name: {:#?}", short_entry.name());
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

    // 根据名称搜索
    pub fn find_vfile_byname(&self, name: &str) -> Option<VFile> {
        assert!(self.is_dir());
        let mut split_name: Vec<&str> = name.split(".").collect();
        if split_name.len() == 1 {
            split_name.push("");
        }
        let name_len = split_name[0].len();
        let extension_len = split_name[1].len();
        let mut short_entry = ShortDirectoryEntry::default();
        self.fs.read().data_manager_modify().read_short_dirent(
            self.short_cluster,
            self.short_offset,
            |entry: &ShortDirectoryEntry| short_entry = *entry,
        );
        if name_len > 8 || extension_len > 3 {
            // 长文件名
            return None; //self.find_long_name(name, short_ent);
        } else {
            // 短文件名
            let name = name.to_uppercase();
            return self.find_short_name(&name, &short_entry);
        }
    }

    // 根据路径递归搜索, 需要区分是绝对路径还是相对路径
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
    //             current_vfile = &vfile;
    //         } else {
    //             return None;
    //         }
    //     }
    //     Some(Arc::new(current_vfile))
    // }
}

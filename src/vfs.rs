use super::{BlockDevice, FileAttributes, LongDirectoryEntry, RunFileSystem, ShortDirectoryEntry};
use spin::RwLock;
use std::sync::Arc;

#[derive(Clone)]
pub struct VFile {
    name: String,
    short_cluster: usize,
    short_offset: usize,               //文件短目录项所在扇区和偏移
    long_pos_vec: Vec<(usize, usize)>, // 长目录项的位置<cluster, offset>
    attribute: FileAttributes,
    fs: Arc<RwLock<RunFileSystem>>,
    block_device: Arc<dyn BlockDevice>,
}

impl VFile {
    pub fn new(
        name: String,
        short_cluster: usize,
        short_offset: usize,
        long_pos_vec: Vec<(usize, usize)>,
        attribute: FileAttributes,
        fs: Arc<RwLock<RunFileSystem>>,
        block_device: Arc<dyn BlockDevice>,
    ) -> Self {
        Self {
            name,
            short_cluster,
            short_offset,
            long_pos_vec,
            attribute,
            fs,
            block_device,
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
    pub fn read_short_dirent<V>(&self, f: impl FnOnce(&ShortDirectoryEntry) -> V) -> V {
        let runfs = self.fs.read();
        if self.short_cluster == runfs.bpb().root_dir_cluster() as usize {
            let root_dirent = runfs.root_dirent();
            let rr = root_dirent.read();
            f(&rr)
        } else {
            runfs
                .data_manager_modify()
                .read_cluster_at(self.short_cluster, self.short_offset, f)
        }
    }
    pub fn modify_short_dirent<V>(&self, f: impl FnOnce(&mut ShortDirectoryEntry) -> V) -> V {
        let runfs = self.fs.read();
        if self.short_cluster == runfs.bpb().root_dir_cluster() as usize {
            let root_dirent = runfs.root_dirent();
            let mut rw = root_dirent.write();
            f(&mut rw)
        } else {
            runfs
                .data_manager_modify()
                .write_cluster_at(self.short_cluster, self.short_offset, f)
        }
    }
    fn modify_long_dirent<V>(
        &self,
        index: usize,
        f: impl FnOnce(&mut LongDirectoryEntry) -> V,
    ) -> V {
        let runfs = self.fs.read();
        let (cluster, offset) = self.long_pos_vec[index];
        runfs
            .data_manager_modify()
            .write_cluster_at(cluster, offset, f)
    }
    pub fn first_cluster(&self) -> u32 {
        self.read_short_dirent(|se: &ShortDirectoryEntry| se.first_cluster())
    }

    pub fn set_first_cluster(&self, clu: u32) {
        self.modify_short_dirent(|se: &mut ShortDirectoryEntry| {
            se.set_first_cluster(clu);
        })
    }
    pub fn read_at(&self, offset: usize, buf: &mut [u8]) -> usize {
        self.read_short_dirent(|short_ent: &ShortDirectoryEntry| {
            short_ent.read_at(
                offset,
                buf,
                &self.fs,
                &self.fs.read().get_fat(),
                &self.block_device,
            )
        })
    }
    pub fn write_at(&self, offset: usize, buf: &[u8]) -> usize {
        self.increase_size((offset + buf.len()) as u32);
        self.modify_short_dirent(|short_ent: &mut ShortDirectoryEntry| {
            short_ent.write_at(
                offset,
                buf,
                &self.fs,
                &self.fs.read().get_fat(),
                &self.block_device,
            )
        })
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

    /* WAITING 目前只支持删除自己*/
    pub fn remove(&self) -> usize {
        let first_cluster: u32 = self.first_cluster();
        for i in 0..self.long_pos_vec.len() {
            self.modify_long_dirent(i, |long_ent: &mut LongDirectoryEntry| {
                long_ent.delete();
            });
        }
        self.modify_short_dirent(|short_ent: &mut ShortDirectoryEntry| {
            short_ent.delete();
        });
        let all_clusters = self
            .fs
            .read()
            .get_fat()
            .read()
            .get_all_cluster_of(first_cluster, self.block_device.clone());
        self.fs.write().dealloc_cluster(all_clusters.clone());
        return all_clusters.len();
    }
}

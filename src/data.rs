use super::{
    BiosParameterBlock, BlockDevice, ClusterCacheManager, LongDirectoryEntry, ShortDirectoryEntry,
};
#[cfg(not(feature = "std"))]
use alloc::sync::Arc;
use spin::RwLock;
#[cfg(feature = "std")]
use std::sync::Arc;

pub struct DataManager {
    root_dirent: Arc<RwLock<ShortDirectoryEntry>>, // 根目录项
    cluster_cache: ClusterCacheManager,
}

impl DataManager {
    pub(crate) fn new(
        bpb: Arc<BiosParameterBlock>,
        root_dirent: Arc<RwLock<ShortDirectoryEntry>>,
        block_device: Arc<dyn BlockDevice>,
    ) -> DataManager {
        Self {
            root_dirent,
            cluster_cache: ClusterCacheManager::new(bpb, block_device),
        }
    }
    pub fn root_dirent(&self) -> Arc<RwLock<ShortDirectoryEntry>> {
        self.root_dirent.clone()
    }
    // /// buf 长度必须比簇 cache 大
    // pub fn read_cluster(&mut self, cluster_id: usize, buf: &mut [u8]) {
    //     let cache = self.cluster_cache.get_cache(cluster_id);
    //     let len = cache.read().len();
    //     for i in 0..len {
    //         cache.write().read(i, |d: &u8| buf[i] = *d);
    //     }
    // }
    // /// buf 长度必须比簇 cache 大
    // pub fn write_cluster(&mut self, cluster_id: usize, buf: &[u8]) {
    //     let cache = self.cluster_cache.get_cache(cluster_id);
    //     let len = cache.read().len();
    //     for i in 0..len {
    //         cache.write().modify(i, |d: &mut u8| *d = buf[i]);
    //     }
    // }

    /// buf 长度必须比簇 cache 大
    pub fn read_cluster(&mut self, cluster_id: usize, buf: &mut [u8]) {
        let cache = self.cluster_cache.get_cache(cluster_id);
        let usize_size = core::mem::size_of::<usize>();
        let usize_len = cache.read().len() / usize_size;
        for i in 0..usize_len {
            cache.write().read(i * usize_size, |d: &usize| unsafe {
                *(&buf[i * usize_size] as *const u8 as usize as *mut usize) = *d;
            });
        }
    }
    /// buf 长度必须比簇 cache 大
    pub fn write_cluster(&mut self, cluster_id: usize, buf: &[u8]) {
        let cache = self.cluster_cache.get_cache(cluster_id);
        let usize_size = core::mem::size_of::<usize>();
        let usize_len = cache.read().len() / usize_size;
        for i in 0..usize_len {
            cache.write().modify(i * usize_size, |d: &mut usize| {
                *d = unsafe { *(&buf[i * usize_size] as *const u8 as usize as *const usize) }
            });
        }
    }
    pub fn clear_cluster(&mut self, cluster_id: usize) {
        let cache = self.cluster_cache.get_cache(cluster_id);
        let usize_size = core::mem::size_of::<usize>();
        let usize_len = cache.read().len() / usize_size;
        for i in 0..usize_len {
            cache.write().modify(i * usize_size, |d: &mut usize| *d = 0);
        }
    }
    pub fn read_cluster_at<T, V>(
        &mut self,
        cluster_id: usize,
        offset: usize,
        f: impl FnOnce(&T) -> V,
    ) -> V
// where
    //     T: ?Sized,
    {
        let cache = self.cluster_cache.get_cache(cluster_id);
        let cache_read = cache.read();
        let cache_ref = cache_read.get_ref(offset);
        f(cache_ref)
    }
    pub fn write_cluster_at<T, V>(
        &mut self,
        cluster_id: usize,
        offset: usize,
        f: impl FnOnce(&mut T) -> V,
    ) -> V {
        let cache = self.cluster_cache.get_cache(cluster_id);
        let mut cache_write = cache.write();
        let cache_mut = cache_write.get_mut(offset);
        f(cache_mut)
    }
    pub fn read_short_dirent<V>(
        &mut self,
        cluster_id: usize,
        offset: usize,
        f: impl FnOnce(&ShortDirectoryEntry) -> V,
    ) -> V {
        self.read_cluster_at(cluster_id, offset, f)
    }
    pub fn modify_short_dirent<V>(
        &mut self,
        cluster_id: usize,
        offset: usize,
        f: impl FnOnce(&mut ShortDirectoryEntry) -> V,
    ) -> V {
        self.write_cluster_at(cluster_id, offset, f)
    }
    pub fn read_long_dirent<V>(
        &mut self,
        cluster_id: usize,
        offset: usize,
        f: impl FnOnce(&LongDirectoryEntry) -> V,
    ) -> V {
        self.read_cluster_at(cluster_id, offset, f)
    }
    pub fn modify_long_dirent<V>(
        &mut self,
        cluster_id: usize,
        offset: usize,
        f: impl FnOnce(&mut LongDirectoryEntry) -> V,
    ) -> V {
        self.write_cluster_at(cluster_id, offset, f)
    }
}


// impl Drop for DataManager {
//     fn drop(&mut self) {
//         self.cluster_cache.data_cache_sync_all();
//     }
// }

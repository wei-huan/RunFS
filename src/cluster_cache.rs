/// 簇缓存层，扇区的进一步抽象，用于 FAT32 的数据区
use super::{BlockDevice, RunFileSystem, CLU_CACHE_SZ, MAX_CLUS_SZ, START_CLUS_ID};
use lazy_static::*;
use spin::RwLock;
use std::collections::VecDeque;
use std::sync::{Arc, Weak};

pub struct ClusterCache {
    cache: Vec<u8>,
    cluster_id: usize, // cluster_id 是数据区的簇号, 一般从 2 开始标号
    modified: bool,
    block_dev: Arc<dyn BlockDevice>, // Arc + dyn 实现 BlockDevice Trait 的动态分发
    file_system: Weak<RunFileSystem>,
}

impl ClusterCache {
    pub fn new(
        cluster_id: usize,
        block_dev: Arc<dyn BlockDevice>,
        runfs: Arc<RunFileSystem>,
    ) -> Self {
        let total_clusters: usize = runfs.bpb.total_clusters().try_into().unwrap();
        let end_cluster_id: usize = total_clusters + START_CLUS_ID;
        assert!(
            cluster_id >= START_CLUS_ID && cluster_id <= end_cluster_id,
            "cluster id not in data range"
        );
        let sectors_per_cluster: usize = runfs.bpb.sectors_per_cluster().try_into().unwrap();
        let data_start_sector: usize = runfs.bpb.first_data_sector().try_into().unwrap();
        let sector_size: usize = runfs.bpb.bytes_per_sector().try_into().unwrap();
        let mut cache: Vec<u8> = vec![0; MAX_CLUS_SZ];
        let block_id = (cluster_id - START_CLUS_ID) * sectors_per_cluster + data_start_sector;
        let cluster_size: usize = runfs.bpb.cluster_size().try_into().unwrap();
        for (i, id) in (block_id..(block_id + sectors_per_cluster)).enumerate() {
            block_dev
                .read_block(id, &mut cache[(i * sector_size)..((i + 1) * sector_size)])
                .unwrap();
        }
        // 先占后缩,适配尽可能宽的簇大小范围,同时避免空间不够用
        cache.resize_with(cluster_size, Default::default);
        cache.shrink_to(cluster_size);
        assert!(
            cache.capacity() == cluster_size,
            "cluster cache len cannot be shrink to proper size"
        );
        Self {
            cache,
            cluster_id,
            modified: false,
            block_dev: block_dev,
            file_system: Arc::downgrade(&runfs),
        }
    }
    pub fn get_cache_ref(&self) -> &[u8] {
        &self.cache
    }
    pub fn get_cache_mut(&mut self) -> &mut [u8] {
        &mut self.cache
    }
    pub fn get_ref<T>(&self, offset: usize) -> &T
    where
        T: Sized,
    {
        let type_size = core::mem::size_of::<T>();
        let cluster_size: usize = self
            .file_system
            .upgrade()
            .unwrap()
            .bpb
            .cluster_size()
            .try_into()
            .unwrap();
        assert!(offset + type_size <= cluster_size);
        unsafe {
            &*((&self.cache[offset..offset + type_size]).as_ptr() as *const _ as usize as *const T)
                as &T
        }
    }
    pub fn get_mut<T>(&mut self, offset: usize) -> &mut T
    where
        T: Sized,
    {
        let type_size = core::mem::size_of::<T>();
        let cluster_size: usize = self
            .file_system
            .upgrade()
            .unwrap()
            .bpb
            .cluster_size()
            .try_into()
            .unwrap();
        assert!(offset + type_size <= cluster_size);
        self.set_modify();
        unsafe {
            &mut *((&mut (self.cache[offset..offset + type_size])).as_mut_ptr() as *mut _ as usize
                as *mut T) as &mut T
        }
    }
    pub fn read<T, V>(&self, offset: usize, f: impl FnOnce(&T) -> V) -> V {
        f(self.get_ref(offset))
    }
    pub fn modify<T, V>(&mut self, offset: usize, f: impl FnOnce(&mut T) -> V) -> V {
        f(self.get_mut(offset))
    }
    pub fn is_modify(&self) -> bool {
        self.modified
    }
    pub fn set_modify(&mut self) {
        self.modified = true
    }
    pub fn sync(&mut self) {
        if self.modified {
            let runfs = self.file_system.upgrade().unwrap();
            let sector_size: usize = runfs.bpb.bytes_per_sector().try_into().unwrap();
            let sectors_per_cluster: usize = runfs.bpb.sectors_per_cluster().try_into().unwrap();
            let data_start_sector: usize = runfs.bpb.first_data_sector().try_into().unwrap();
            self.modified = false;
            let block_id =
                (self.cluster_id - START_CLUS_ID) * sectors_per_cluster + data_start_sector;
            for (i, id) in (block_id..(block_id + sectors_per_cluster)).enumerate() {
                self.block_dev
                    .write_block(id, &self.cache[(i * sector_size)..((i + 1) * sector_size)])
                    .unwrap();
            }
        }
    }
}

impl Drop for ClusterCache {
    fn drop(&mut self) {
        self.sync()
    }
}

pub struct ClusterCacheManager {
    pub(crate) file_system: Option<Arc<RunFileSystem>>,
    queue: VecDeque<(usize, Arc<RwLock<ClusterCache>>)>,
}

impl ClusterCacheManager {
    pub fn new() -> Self {
        Self {
            file_system: None,
            queue: VecDeque::new(),
        }
    }
    pub fn set_fs(&mut self, runfs: Arc<RunFileSystem>) {
        self.file_system = Some(runfs);
    }
    pub fn get_cache(
        &mut self,
        cluster_id: usize,
        block_device: Arc<dyn BlockDevice>,
    ) -> Arc<RwLock<ClusterCache>> {
        if let Some(pair) = self.queue.iter().find(|pair| pair.0 == cluster_id) {
            Arc::clone(&pair.1)
        } else {
            // substitute
            if self.queue.len() == CLU_CACHE_SZ {
                // from front to tail
                if let Some((idx, _)) = self
                    .queue
                    .iter()
                    .enumerate()
                    .find(|(_, pair)| Arc::strong_count(&pair.1) == 1)
                {
                    self.queue.drain(idx..=idx);
                } else {
                    panic!("Run out of SectorCache!");
                }
            }
            // load cluster into mem and push back
            let cluster_cache = Arc::new(RwLock::new(ClusterCache::new(
                cluster_id,
                Arc::clone(&block_device),
                Arc::clone(&self.file_system.as_ref().unwrap()),
            )));
            self.queue
                .push_back((cluster_id, Arc::clone(&cluster_cache)));
            cluster_cache
        }
    }
    pub fn data_cache_sync_all(&mut self) {
        for (_, cache) in self.queue.iter() {
            cache.write().sync();
        }
    }
}

lazy_static! {
    pub static ref DATA_CLUS_CACHE_MANAGER: RwLock<ClusterCacheManager> =
        RwLock::new(ClusterCacheManager::new());
}

pub fn get_data_cache(
    cluster_id: usize,
    block_device: Arc<dyn BlockDevice>,
) -> Arc<RwLock<ClusterCache>> {
    DATA_CLUS_CACHE_MANAGER
        .write()
        .get_cache(cluster_id, block_device)
}

pub fn data_cache_sync_all() {
    DATA_CLUS_CACHE_MANAGER.write().data_cache_sync_all();
}

pub fn data_cache_set_fs(runfs: Arc<RunFileSystem>) {
    DATA_CLUS_CACHE_MANAGER.write().set_fs(runfs);
}
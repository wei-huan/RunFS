/// 簇缓存层，扇区的进一步抽象，用于 FAT32 的数据区
use super::{
    BlockDevice, SectorCache, CLUS_SZ, CLU_CACHE_SZ, DATA_START_SEC, SECS_PER_CLU, SEC_SZ,
};
use lazy_static::*;
use spin::RwLock;
use std::collections::VecDeque;
use std::sync::Arc;

pub struct ClusterCache {
    cache: [u8; CLUS_SZ],
    cluster_id: usize,               // cluster_id 是数据区的簇号, 从 2 开始标号
    block_dev: Arc<dyn BlockDevice>, // Arc + dyn 实现 BlockDevice Trait 的动态分发
    modified: bool,
}

impl ClusterCache {
    pub fn new(cluster_id: usize, block_dev: Arc<dyn BlockDevice>) -> Self {
        assert!(cluster_id >= 2);
        let mut cache = [0u8; CLUS_SZ];
        let block_id = (cluster_id - 2) * SECS_PER_CLU + DATA_START_SEC;
        for (i, id) in (block_id..(block_id + SECS_PER_CLU)).enumerate() {
            block_dev.read_block(id, &mut cache[(i * SEC_SZ)..((i + 1) * SEC_SZ)]);
        }
        Self {
            cache,
            cluster_id,
            modified: false,
            block_dev: block_dev,
        }
    }
    pub fn get_ref<T>(&self, offset: usize) -> &T
    where
        T: Sized,
    {
        let type_size = core::mem::size_of::<T>();
        assert!(offset + type_size <= CLUS_SZ);
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
        assert!(offset + type_size <= CLUS_SZ);
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
            self.modified = false;
            let block_id = (self.cluster_id - 2) * SECS_PER_CLU + DATA_START_SEC;
            for (i, id) in (block_id..(block_id + SECS_PER_CLU)).enumerate() {
                self.block_dev
                    .write_block(id, &self.cache[(i * SEC_SZ)..((i + 1) * SEC_SZ)]);
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
    queue: VecDeque<(usize, Arc<RwLock<ClusterCache>>)>,
}

impl ClusterCacheManager {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
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
            )));
            self.queue
                .push_back((cluster_id, Arc::clone(&cluster_cache)));
            cluster_cache
        }
    }
}

lazy_static! {
    pub static ref DATA_CLUS_MANAGER: RwLock<ClusterCacheManager> =
        RwLock::new(ClusterCacheManager::new());
}

pub fn get_data_cache(
    cluster_id: usize,
    block_device: Arc<dyn BlockDevice>,
) -> Arc<RwLock<ClusterCache>> {
    assert!(cluster_id >= 2);
    DATA_CLUS_MANAGER
        .write()
        .get_cache(cluster_id, block_device)
}

pub fn data_cache_sync_all() {
    let manager = DATA_CLUS_MANAGER.write();
    for (_, cache) in manager.queue.iter() {
        cache.write().sync();
    }
}

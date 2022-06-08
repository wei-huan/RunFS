/// 块缓存层，用于 FAT32 的保留扇区和 FAT 表
use super::{BlockDevice, BLOCK_SZ, DATA_END_SEC, DATA_START_SEC, MAX_SEC_SZ, INFOSEC_CACHE_SZ};
use lazy_static::*;
use spin::RwLock;
use std::collections::VecDeque;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

// 在本系统设计中, BlockCache块缓存被认为是硬件存储的最小分配单元,逻辑上来说不是文件系统读取的最小单位.
pub struct BlockCache {
    cache: Vec<u8>,
    sector_id: usize,
    modified: bool,
    block_dev: Arc<dyn BlockDevice>, // Arc + dyn 实现 BlockDevice Trait 的动态分发
}

impl BlockCache {
    pub fn new(sector_id: usize, sector_size: usize, block_dev: Arc<dyn BlockDevice>) -> Self {
        assert!((sector_id >= DATA_START_SEC) && (sector_id <= DATA_END_SEC));
        let mut cache: Vec<u8> = Vec::with_capacity(MAX_SEC_SZ);
        block_dev.read_block(sector_id, &mut cache);
        // 先占后缩,适配尽可能宽的簇大小范围,同时避免空间不够用
        cache.shrink_to(sector_size);
        assert!(cache.capacity() == sector_size);
        Self {
            cache,
            sector_id,
            modified: false,
            block_dev: block_dev,
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
        assert!(offset + type_size <= BLOCK_SZ);
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
        assert!(offset + type_size <= BLOCK_SZ);
        self.set_modify();
        unsafe {
            &mut *((&mut (self.cache[offset..offset + type_size])).as_mut_ptr() as *mut _ as usize
                as *mut T) as &mut T
        }
    }
    pub fn read<T, U>(&self, offset: usize, f: impl FnOnce(&T) -> U) -> U {
        f(self.get_ref(offset))
    }
    pub fn modify<T, U>(&mut self, offset: usize, f: impl FnOnce(&mut T) -> U) -> U {
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
            self.block_dev
                .write_block(self.sector_id, self.cache.as_ref());
        }
    }
}

impl Drop for BlockCache {
    fn drop(&mut self) {
        self.sync()
    }
}

// 在本系统设计中, 文件系统最小读取单位SectorCache等于硬件最小的分配单元.
pub type SectorCache = BlockCache;

pub struct SectorCacheManager {
    queue: VecDeque<(usize, Arc<RwLock<SectorCache>>)>,
}

impl SectorCacheManager {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }
    pub fn get_cache(
        &mut self,
        sector_id: usize,
        block_device: Arc<dyn BlockDevice>,
    ) -> Arc<RwLock<SectorCache>> {
        if let Some(pair) = self.queue.iter().find(|pair| pair.0 == sector_id) {
            Arc::clone(&pair.1)
        } else {
            // substitute
            if self.queue.len() == INFOSEC_CACHE_SZ {
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
            // load sector into mem and push back
            let sector_cache = Arc::new(RwLock::new(BlockCache::new(
                512,
                sector_id,
                Arc::clone(&block_device),
            )));
            self.queue.push_back((sector_id, Arc::clone(&sector_cache)));
            sector_cache
        }
    }
}

lazy_static! {
    pub static ref INFOSEC_CACHE_MANAGER: RwLock<SectorCacheManager> =
        RwLock::new(SectorCacheManager::new());
}

pub fn get_info_cache(
    sector_id: usize,
    block_device: Arc<dyn BlockDevice>,
) -> Arc<RwLock<SectorCache>> {
    assert!(sector_id < DATA_START_SEC);
    INFOSEC_CACHE_MANAGER
        .write()
        .get_cache(sector_id, block_device)
}

pub fn info_cache_sync_all() {
    let manager = INFOSEC_CACHE_MANAGER.write();
    for (_, cache) in manager.queue.iter() {
        cache.write().sync();
    }
}

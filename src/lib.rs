#![allow(unused)]

mod block_device;
mod cluster_cache;
mod directory_entry;
mod error;
mod file_alloc_table;
mod runfs;
mod sector_cache;

use sector_cache::SectorCache;

pub use block_device::BlockDevice;
pub use cluster_cache::{data_cache_sync_all, get_data_cache};
pub use sector_cache::{get_info_cache, info_cache_sync_all};

pub const BLOCK_SZ: usize = 0x200;
pub const SEC_SZ: usize = BLOCK_SZ;
pub const MAX_SEC_SZ: usize = SEC_SZ * 8; // 限制最大扇区4096Byte,太大了不伺候了,单片机受不了
const SECS_PER_CLU: usize = 0x10;
pub const CLUS_SZ: usize = SEC_SZ * SECS_PER_CLU;
pub const MAX_CLUS_SZ: usize = SEC_SZ * 64; // 限制最大簇32KB,太大了不伺候了,单片机受不了
const START_CLUS_ID: usize = 0x02;
const RESERVE_SEC_SZ: usize = 32;
const TOTAL_SECS: usize = 256000;
const FAT_TABLE_SECS: usize = 1969;
const DBR_START_SEC: usize = 0;

pub const FAT_TABLE_START_SEC: usize = DBR_START_SEC + RESERVE_SEC_SZ;
pub const DATA_START_SEC: usize = FAT_TABLE_START_SEC + FAT_TABLE_SECS * 2;
pub const DATA_END_SEC: usize = FAT_TABLE_START_SEC + FAT_TABLE_SECS * 2;

// 扇区缓冲区长度
const INFOSEC_CACHE_SZ: usize = 4;

// 簇缓冲区长度
const CLU_CACHE_SZ: usize = 2;

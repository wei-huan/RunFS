mod block_device;
mod boot_sector;
mod cluster_cache;
mod directory_entry;
mod error;
mod file_alloc_table;
mod fsinfo;
mod runfs;
mod sector_cache;

use sector_cache::SectorCache;
use fsinfo::{FSInfoSector, FSInfo};

pub use error::{FSError, IOError};
pub use boot_sector::{BootSector, BiosParameterBlock};
pub use block_device::BlockDevice;
pub use cluster_cache::{data_cache_sync_all, get_data_cache};
pub use sector_cache::{get_info_cache, info_cache_sync_all};
pub use runfs::RunFileSystem;

pub const MAX_SEC_SZ: usize = 4096; // 限制最大扇区4096Byte, 太大了不伺候了, 单片机受不了
pub const MAX_CLUS_SZ: usize = 512 * 64; // 限制最大簇32KB, 太大了不伺候了, 单片机受不了
pub const START_CLUS_ID: usize = 2;

// 扇区缓冲区长度
const INFOSEC_CACHE_SZ: usize = 4;

// 簇缓冲区长度
const CLU_CACHE_SZ: usize = 2;

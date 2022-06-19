// #![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

mod block_device;
mod boot_sector;
mod cluster_cache;
mod config;
mod data;
mod dir_entry;
mod error;
mod fat;
mod fsinfo;
mod runfs;
mod sector_cache;
mod vfs;

use cluster_cache::ClusterCacheManager;
use data::DataManager;
use dir_entry::{
    LongDirectoryEntry, ShortDirectoryEntry, DIRENT_SZ, LAST_LONG_ENTRY, LONG_NAME_LEN,
    SHORT_FILE_EXT_LEN, SHORT_FILE_NAME_LEN, SHORT_FILE_NAME_PADDING, SHORT_NAME_LEN,
};
use fat::FATManager;
use fsinfo::{FSInfo, FSInfoSector};
use sector_cache::SectorCacheManager;

pub use block_device::BlockDevice;
pub use boot_sector::{BiosParameterBlock, BootSector};
pub use dir_entry::FileAttributes;
pub use error::{FSError, IOError};
pub use fat::FATEntry;
pub use runfs::RunFileSystem;
pub use vfs::{long_name_split, VFile};
pub const START_CLUS_ID: usize = 2;

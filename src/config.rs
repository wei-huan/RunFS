/// 可以自行调整的变量

pub const MAX_SEC_SZ: usize = 4096; // 限制最大扇区4096Byte, 太大了单片机受不了
pub const MAX_CLUS_SZ: usize = 512 * 64; // 限制最大簇32KB, 太大了单片机受不了

pub const INFOSEC_CACHE_SZ: usize = 4; // 扇区缓冲区长度
pub const DATACLU_CACHE_SZ: usize = 2; // 簇缓冲区长度

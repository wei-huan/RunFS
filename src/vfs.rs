use super::{BlockDevice, RunFileSystem};
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct VFile {
    name: String,
    short_sector: usize,
    short_offset: usize,               //文件短目录项所在扇区和偏移
    long_pos_vec: Vec<(usize, usize)>, // 长目录项的位置<sector, offset>
    attribute: u8,
    fs: Arc<RwLock<RunFileSystem>>,
    block_device: Arc<dyn BlockDevice>,
}

impl VFile {
    pub fn new(
        name: String,
        short_sector: usize,
        short_offset: usize,
        long_pos_vec: Vec<(usize, usize)>,
        attribute: u8,
        fs: Arc<RwLock<RunFileSystem>>,
        block_device: Arc<dyn BlockDevice>,
    ) -> Self {
        Self {
            name,
            short_sector,
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
    pub fn attribute(&self) -> u8 {
        self.attribute
    }
    pub fn fs(&self) -> Arc<RwLock<RunFileSystem>> {
        self.fs.clone()
    }
}

use runfs::{BlockDevice, IOError, RunFileSystem, VFile};
use spin::RwLock;
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::{prelude::*, Seek, SeekFrom};
use std::sync::Arc;

struct FileEmulateBlockDevice {
    path: String,
}

impl FileEmulateBlockDevice {
    fn new(path: String) -> Self {
        Self { path }
    }
}

const BLOCK_SZ: usize = 512;

impl BlockDevice for FileEmulateBlockDevice {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) -> Result<(), IOError> {
        let _metadata = fs::metadata(self.path.as_str()).expect("Open Img Failed");
        // let file_size: usize = metadata.len().try_into().unwrap();
        let mut file = File::open(self.path.as_str()).expect("No Img");
        // println!("block_id: {}", block_id);
        let pos: usize = block_id * BLOCK_SZ;
        // println!("pos: {}", pos);
        // println!("file_size: {}", file_size);
        // assert!(pos + BLOCK_SZ < file_size); // 如果是块设备就算了
        file.seek(SeekFrom::Start(pos.try_into().unwrap()))
            .expect("Seek Failed");
        file.read(buf).unwrap();
        Ok(())
    }
    fn write_block(&self, block_id: usize, buf: &[u8]) -> Result<(), IOError> {
        let _metadata = fs::metadata(self.path.as_str()).expect("Open Img Failed");
        // let file_size: usize = metadata.len().try_into().unwrap();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(self.path.as_str())
            .expect("No Img");
        let pos: usize = block_id * BLOCK_SZ;
        // assert!(pos + BLOCK_SZ < file_size);
        file.seek(SeekFrom::Start(pos.try_into().unwrap()))
            .expect("Seek Failed");
        file.write(buf).expect("Write Failed");
        Ok(())
    }
}

const IMG: &str = "assets/fat32_1.img";

#[test]
fn test_find_file_short() {
    let file_block_device: FileEmulateBlockDevice = FileEmulateBlockDevice::new(IMG.to_string());
    let runfs = Arc::new(RwLock::new(RunFileSystem::new(Arc::new(file_block_device))));
    let root_dir: Arc<VFile> = Arc::new(runfs.read().root_vfile(&runfs));
    let vfile = root_dir.find_vfile_byname("FUCK").unwrap();
    println!("file: {:#X?}", vfile.name());
}

#[test]
fn test_find_file_long() {
    let file_block_device: FileEmulateBlockDevice = FileEmulateBlockDevice::new(IMG.to_string());
    let runfs = Arc::new(RwLock::new(RunFileSystem::new(Arc::new(file_block_device))));
    let root_dir: Arc<VFile> = Arc::new(runfs.read().root_vfile(&runfs));
    let vfile = root_dir.find_vfile_byname("mount").unwrap();
    println!("file: {:#X?}", vfile.name());
    println!("file long_pos: {:#?}", vfile.long_pos());
    println!("file short_pos: {:#?}", vfile.short_pos());
}

#[test]
fn test_delete_file() {
    let file_block_device: FileEmulateBlockDevice = FileEmulateBlockDevice::new(IMG.to_string());
    let runfs = Arc::new(RwLock::new(RunFileSystem::new(Arc::new(file_block_device))));
    let root_dir: Arc<VFile> = Arc::new(runfs.read().root_vfile(&runfs));
    let vfile = root_dir.find_vfile_byname("mount").unwrap();
    println!("file: {:#X?}", vfile.name());
    vfile.delete();
}

use runfs::{BlockDevice, FATEntry, IOError, RunFileSystem};
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

const CLUSTER_ID: usize = 2;
const IMG: &str = "/dev/sda";

#[test]
fn read_fat() {
    let file_block_device: FileEmulateBlockDevice = FileEmulateBlockDevice::new(IMG.to_string());
    let runfs = RunFileSystem::new(Arc::new(file_block_device));
    let entry = runfs.fat_manager_modify().entry(CLUSTER_ID);
    println!("entry: {:#X?}", entry);
}
#[test]
fn write_fat() {
    let file_block_device: FileEmulateBlockDevice = FileEmulateBlockDevice::new(IMG.to_string());
    let runfs = RunFileSystem::new(Arc::new(file_block_device));
    runfs
        .fat_manager_modify()
        .set_entry(CLUSTER_ID, FATEntry::Free);
    let entry = runfs.fat_manager_modify().entry(CLUSTER_ID);
    println!("entry: {:#X?}", entry);
}

#[test]
fn test_fat_fsinfo() {
    let file_block_device: FileEmulateBlockDevice =
        FileEmulateBlockDevice::new(IMG.to_string());
    let runfs = RunFileSystem::new(Arc::new(file_block_device));
    let mut fat_manager = runfs.fat_manager_modify();
    let available = fat_manager.search_free_cluster(CLUSTER_ID);
    println!("available: {:#X?}", available);
    assert_eq!(available, fat_manager.fsinfo().next_free_cluster());
    let free_count = fat_manager.count_free_clusters();
    assert_eq!(free_count, fat_manager.fsinfo().free_clusters().unwrap());
}

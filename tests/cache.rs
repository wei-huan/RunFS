use runfs::{BlockDevice, IOError, RunFileSystem};
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
const IMG: &str = "assets/fat32_1.img";

#[test]
fn read_entry() {
    let file_block_device: FileEmulateBlockDevice = FileEmulateBlockDevice::new(IMG.to_string());
    let runfs = RunFileSystem::new(Arc::new(file_block_device));
    let mut fat_manager = runfs.fat_manager_modify();
    let next_cluster = fat_manager.next_cluster(CLUSTER_ID);
    println!("next_cluster: {:#X?}", next_cluster);
    let rootdir_entrys = fat_manager.all_clusters(CLUSTER_ID);
    println!("rootdir_entrys: {:#X?}", rootdir_entrys);
    let num = fat_manager.count_clusters(CLUSTER_ID);
    println!("num: {:#?}", num);
}

#[test]
fn test_alloc_cluster() {
    let file_block_device: FileEmulateBlockDevice = FileEmulateBlockDevice::new(IMG.to_string());
    let runfs = RunFileSystem::new(Arc::new(file_block_device));
    let mut fat_manager = runfs.fat_manager_modify();
    let available = fat_manager.fsinfo().free_clusters();
    println!("available: {:#X?}", available);
    let next = fat_manager.fsinfo().next_free_cluster();
    println!("next: {:#X?}", next);
    let alloc = fat_manager.alloc_cluster(None);
    println!("alloc: {:#X?}", alloc);
}

#[test]
fn test_alloc_clusters() {
    let file_block_device: FileEmulateBlockDevice = FileEmulateBlockDevice::new(IMG.to_string());
    let runfs = RunFileSystem::new(Arc::new(file_block_device));
    let mut fat_manager = runfs.fat_manager_modify();
    let available = fat_manager.fsinfo().free_clusters();
    println!("available: {:#X?}", available);
    let next = fat_manager.fsinfo().next_free_cluster();
    println!("next: {:#X?}", next);
    let alloc = fat_manager.alloc_clusters(0x100, None);
    println!("alloc: {:#X?}", alloc);
    let alloc_chain = fat_manager.all_clusters(alloc.unwrap() as usize);
    println!("alloc_chain: {:#X?}", alloc_chain);
}

#[test]
fn test_clear_cluster() {
    let file_block_device: FileEmulateBlockDevice = FileEmulateBlockDevice::new(IMG.to_string());
    let runfs = RunFileSystem::new(Arc::new(file_block_device));
    let fat_manager = runfs.fat_manager_read();
    let next = fat_manager.fsinfo().next_free_cluster();
    println!("next: {:#X?}", next);
    let mut data_manager = runfs.data_manager_modify();
    let mut buffer = [0u8; 512];
    data_manager.read_cluster(next.unwrap() as usize, &mut buffer);
    println!("buffer before clear: {:X?}", buffer);
    data_manager.clear_cluster(next.unwrap() as usize);
    data_manager.read_cluster(next.unwrap() as usize, &mut buffer);
    println!("buffer after clear: {:X?}", buffer);
}

#[test]
fn test_fs_alloc_cluster() {
    let file_block_device: FileEmulateBlockDevice = FileEmulateBlockDevice::new(IMG.to_string());
    let mut runfs = RunFileSystem::new(Arc::new(file_block_device));
    let fat_manager = runfs.fat_manager_modify();
    let available = fat_manager.fsinfo().free_clusters();
    println!("available: {:#X?}", available);
    let next = fat_manager.fsinfo().next_free_cluster();
    println!("next: {:#X?}", next);
    drop(fat_manager);
    let id = runfs.alloc_cluster(None).unwrap();
    println!("id: {:#X?}", id);
    let mut buffer = [12u8; 512];
    let mut data_manager = runfs.data_manager_modify();
    data_manager.read_cluster(id as usize, &mut buffer);
    println!("buffer after clear: {:X?}", buffer);
    let fat_manager = runfs.fat_manager_modify();
    let available = fat_manager.fsinfo().free_clusters();
    println!("available: {:#X?}", available);
    let next = fat_manager.fsinfo().next_free_cluster();
    println!("next: {:#X?}", next);
}

#[test]
fn test_fs_alloc_clusters() {
    let file_block_device: FileEmulateBlockDevice = FileEmulateBlockDevice::new(IMG.to_string());
    let mut runfs = RunFileSystem::new(Arc::new(file_block_device));
    let available = runfs.free_clusters();
    println!("available: {:#X?}", available);
    let next = runfs.next_free_cluster();
    println!("next: {:#X?}", next);
    let first_id = runfs.alloc_clusters(0x100, None).unwrap();
    println!("first_id: {:#X?}", first_id);
    let mut buffer = [12u8; 512];
    let mut data_manager = runfs.data_manager_modify();
    data_manager.read_cluster(first_id as usize, &mut buffer);
    println!("buffer after clear: {:X?}", buffer);
    data_manager.read_cluster((first_id + 1) as usize, &mut buffer);
    println!("buffer after clear: {:X?}", buffer);
    data_manager.read_cluster((first_id + 2) as usize, &mut buffer);
    println!("buffer after clear: {:X?}", buffer);
    let available = runfs.free_clusters();
    println!("available: {:#X?}", available);
    let next = runfs.next_free_cluster();
    println!("next: {:#X?}", next);
}

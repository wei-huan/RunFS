use runfs::{long_name_split, BlockDevice, BootSector, IOError, RunFileSystem};
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

const IMG: &str = "../fat32_1.img";
#[test]
fn check_boot_sector() {
    let file_block_device: FileEmulateBlockDevice = FileEmulateBlockDevice::new(IMG.to_string());
    let boot_sector = BootSector::directly_new(Arc::new(file_block_device));
    println!("boot_sector: {:#X?}", boot_sector);
}
#[test]
fn create_file_system() {
    let file_block_device: FileEmulateBlockDevice = FileEmulateBlockDevice::new(IMG.to_string());
    let runfs = RunFileSystem::new(Arc::new(file_block_device));
    println!("BPB: {:#X?}", runfs.bpb());
}
#[test]
fn test_long_name_u16() {
    let name = String::from("这真的是一个文件名很长的文件, 需要切开wakuwaku.txt");
    let utf16 = name.encode_utf16();
    let first: Vec<u16> = utf16.collect();
    let array: Vec<&[u16]> = first.as_slice().chunks(13).collect();
    let name = String::from_utf16_lossy(array[2]);
    println!("name: {:#X?}", name);
}
#[test]
fn test_long_name_split() {
    let name_vec: Vec<[u16; 13]> =
        long_name_split("这是一个文件名很长的文件, 需要切开wakuwaku.txt")
            .into_iter()
            .rev()
            .collect();
    println!("name_vec: {:#?}", String::from_utf16_lossy(&name_vec[0]))
}

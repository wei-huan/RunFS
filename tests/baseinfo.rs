use runfs::{BlockDevice, BootSector, IOError, RunFileSystem};
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
fn check_file_system() {
    let file_block_device: FileEmulateBlockDevice = FileEmulateBlockDevice::new(IMG.to_string());
    let runfs = RunFileSystem::new(Arc::new(file_block_device));
    println!("runfs_size: {:#?}", core::mem::size_of::<RunFileSystem>());
    let bpb = runfs.bpb();
    let fsinfo = runfs.fsinfo();
    println!("fats_sectors: {:#?}", bpb.fats_sectors());
    println!("total_sectors_32: {:#?}", bpb.total_sectors_32());
    println!("reserved_sectors: {:#?}", bpb.reserved_sectors());
    println!("root_dir_sectors: {:#?}", bpb.root_dir_sectors());
    println!("sectors_per_all_fats: {:#?}", bpb.sectors_per_all_fats());
    println!("first_data_sector: {:#?}", bpb.first_data_sector());
    println!("total_clusters: {:#?}", bpb.total_clusters());
    println!("cluster_size: {:#?}", bpb.cluster_size());
    println!("fsinfo_sector: {:#?}", bpb.fsinfo_sector());
    println!("backup_boot_sector: {:#?}", bpb.backup_boot_sector());
    println!("next_free_cluster: {:#?}", fsinfo.next_free_cluster());
    println!("free_cluster_count: {:#?}", fsinfo.free_clusters());
}

use runfs::{BlockDevice, FileAttributes, IOError, RunFileSystem, VFile};
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

// const IMG: &str = "assets/fat32_1.img";
const IMG: &str = "/dev/sda";

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
fn test_directory_size() {
    let file_block_device: FileEmulateBlockDevice = FileEmulateBlockDevice::new(IMG.to_string());
    let runfs = Arc::new(RwLock::new(RunFileSystem::new(Arc::new(file_block_device))));
    let root_dir: Arc<VFile> = Arc::new(runfs.read().root_vfile(&runfs));
    let size = root_dir.size();
    println!("size: {:#?}", size);
}

#[test]
fn test_find_dirents() {
    let file_block_device: FileEmulateBlockDevice = FileEmulateBlockDevice::new(IMG.to_string());
    let runfs = Arc::new(RwLock::new(RunFileSystem::new(Arc::new(file_block_device))));
    let root_dir: Arc<VFile> = Arc::new(runfs.read().root_vfile(&runfs));
    let offset = root_dir.find_free_dirents(3);
    println!("file offset: {:#?}", offset);
}

#[test]
fn test_delete_file() {
    let file_block_device: FileEmulateBlockDevice = FileEmulateBlockDevice::new(IMG.to_string());
    let runfs = Arc::new(RwLock::new(RunFileSystem::new(Arc::new(file_block_device))));
    let root_dir: Arc<VFile> = Arc::new(runfs.read().root_vfile(&runfs));
    let vfile = root_dir.find_vfile_byname("getcwd").unwrap();
    println!("file: {:#X?}", vfile.name());
    vfile.delete();
}

#[test]
fn test_delete_dir() {
    let file_block_device: FileEmulateBlockDevice = FileEmulateBlockDevice::new(IMG.to_string());
    let runfs = Arc::new(RwLock::new(RunFileSystem::new(Arc::new(file_block_device))));
    let root_dir: Arc<VFile> = Arc::new(runfs.read().root_vfile(&runfs));
    let vfile = root_dir.find_vfile_byname("mnt").unwrap();
    println!("file: {:#X?}", vfile.name());
    vfile.delete();
}

#[test]
fn test_create_file() {
    let file_block_device: FileEmulateBlockDevice = FileEmulateBlockDevice::new(IMG.to_string());
    let runfs = Arc::new(RwLock::new(RunFileSystem::new(Arc::new(file_block_device))));
    let root_dir: Arc<VFile> = Arc::new(runfs.read().root_vfile(&runfs));
    // let file =
    root_dir
        .create("helloworld.txt", FileAttributes::FILE)
        .unwrap();
    // let first = file.first_data_cluster();
    // println!("first: {:#X?}", first);
}

#[test]
fn test_create_dir() {
    let file_block_device: FileEmulateBlockDevice = FileEmulateBlockDevice::new(IMG.to_string());
    let runfs = Arc::new(RwLock::new(RunFileSystem::new(Arc::new(file_block_device))));
    let root_dir: Arc<VFile> = Arc::new(runfs.read().root_vfile(&runfs));
    root_dir
        .create("wakuwaku", FileAttributes::DIRECTORY)
        .unwrap();
}

#[test]
fn test_create_dir_in_subdir() {
    let file_block_device: FileEmulateBlockDevice = FileEmulateBlockDevice::new(IMG.to_string());
    let runfs = Arc::new(RwLock::new(RunFileSystem::new(Arc::new(file_block_device))));
    let root_dir: Arc<VFile> = Arc::new(runfs.read().root_vfile(&runfs));
    let vfile = root_dir.find_vfile_byname("wakuwaku").unwrap();
    vfile
        .create("wakuwakuwaku", FileAttributes::DIRECTORY)
        .unwrap();
    println!("file: {:#X?}", vfile.name());
}

#[test]
fn test_create_file_in_subdir() {
    let file_block_device: FileEmulateBlockDevice = FileEmulateBlockDevice::new(IMG.to_string());
    let runfs = Arc::new(RwLock::new(RunFileSystem::new(Arc::new(file_block_device))));
    let root_dir: Arc<VFile> = Arc::new(runfs.read().root_vfile(&runfs));
    let vfile = root_dir.find_vfile_byname("wakuwaku").unwrap();
    let helloworld = vfile
        .create("helloworld.txt", FileAttributes::FILE)
        .unwrap();
    println!("file: {:#X?}", helloworld.name());
}

#[test]
fn test_read_file() {
    use std::time::Instant;
    let file_block_device: FileEmulateBlockDevice = FileEmulateBlockDevice::new(IMG.to_string());
    let runfs = Arc::new(RwLock::new(RunFileSystem::new(Arc::new(file_block_device))));
    let root_dir: Arc<VFile> = Arc::new(runfs.read().root_vfile(&runfs));
    let text = root_dir.find_vfile_byname("open").unwrap();
    let mut buf = [0u8; 62400];
    let start = Instant::now();
    let len = text.read_at(0, &mut buf);
    let duration = start.elapsed();
    println!("Time elapsed is: {:?}", duration);
    // println!("buf: {:?}", buf);
    println!("text len: {:#}", len);
    // let s = String::from_utf8_lossy(&buf);
    // println!("{:#}", s);
}

#[test]
fn test_write_file() {
    let file_block_device: FileEmulateBlockDevice = FileEmulateBlockDevice::new(IMG.to_string());
    let runfs = Arc::new(RwLock::new(RunFileSystem::new(Arc::new(file_block_device))));
    let root_dir: Arc<VFile> = Arc::new(runfs.read().root_vfile(&runfs));
    let mut buf = [0x0u8; 130000];
    let text = root_dir.find_vfile_byname("user_shell").unwrap();
    let len = text.read_at(0, &mut buf);
    println!("text read len: {:#}", len);
    let helloworld = root_dir.find_vfile_byname("helloworld.txt").unwrap();
    println!("Here0");
    let len = helloworld.write_at(0, &buf);
    println!("helloworld write len: {:#}", len);
    let mut buf1 = [0u8; 52];
    let len = helloworld.read_at(0, &mut buf1);
    println!("helloworld read len: {:#}", len);
    let s = String::from_utf8_lossy(&buf1);
    println!("{:#}", s);
}

#[test]
fn test_stat() {
    let file_block_device: FileEmulateBlockDevice = FileEmulateBlockDevice::new(IMG.to_string());
    let runfs = Arc::new(RwLock::new(RunFileSystem::new(Arc::new(file_block_device))));
    let root_dir: Arc<VFile> = Arc::new(runfs.read().root_vfile(&runfs));
    let text = root_dir.find_vfile_byname("user_shell").unwrap();
    let stat = text.stat();
    println!("shell stat: {:#?}", stat);
}

#[test]
fn test_dirent_info() {
    use std::time::Instant;
    let file_block_device: FileEmulateBlockDevice = FileEmulateBlockDevice::new(IMG.to_string());
    let runfs = Arc::new(RwLock::new(RunFileSystem::new(Arc::new(file_block_device))));
    let root_dir: Arc<VFile> = Arc::new(runfs.read().root_vfile(&runfs));
    let start = Instant::now();
    let info = root_dir.dirent_info(128);
    let duration = start.elapsed();
    println!("Time elapsed is: {:?}", duration);
    println!("info: {:#?}", info);
}

#[test]
fn test_ls() {
    use std::time::Instant;
    let file_block_device: FileEmulateBlockDevice = FileEmulateBlockDevice::new(IMG.to_string());
    let runfs = Arc::new(RwLock::new(RunFileSystem::new(Arc::new(file_block_device))));
    let root_dir: Arc<VFile> = Arc::new(runfs.read().root_vfile(&runfs));
    let start = Instant::now();
    let ls = root_dir.ls();
    let duration = start.elapsed();
    println!("Time elapsed is: {:?}", duration);
    println!("ls: {:#?}", ls);
}

#[test]
fn test_find_file_by_path() {
    let file_block_device: FileEmulateBlockDevice = FileEmulateBlockDevice::new(IMG.to_string());
    let runfs = Arc::new(RwLock::new(RunFileSystem::new(Arc::new(file_block_device))));
    let root_dir: Arc<VFile> = Arc::new(runfs.read().root_vfile(&runfs));
    let path = String::from("/initproc");
    let vfile = root_dir.find_vfile_bypath(&path).unwrap();
    println!("vfile: {:#?}", vfile.name());
    // let path_rela = String::from("../ccc/text.txt");
    // let vfile_rela = vfile.find_vfile_bypath(&path_rela).unwrap();
    // println!("vfile: {:#?}", vfile_rela.name());
    // let path = String::from("/initproc");
    // let pos = path.rfind("/").unwrap();
    // let (prev, last) = path.split_at(pos + 1);
    // println!("prev: {:#?}", prev);
    // println!("last: {:#?}", last);
}

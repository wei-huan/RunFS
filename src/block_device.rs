use crate::error::IOError;
use std::any::Any;

pub trait BlockDevice: Send + Sync + Any {
    // TODO: read, write 要返回结果 Result
    // read_block中, 如果 block 长度大于 buf, 必须确保 buf 顺利读到 block 前 n 个的
    //数据, 前n个数据不会被覆盖或者读取失败, 错误在Result中返回处理
    fn read_block(&self, block_id: usize, buf: &mut [u8]) -> Result<(), IOError>;
    // write_block中, 如果 block 长度大于 buf, 必须确保 buf 不会写进 block,直接返回error
    fn write_block(&self, block_id: usize, buf: &[u8]) -> Result<(), IOError>;
}

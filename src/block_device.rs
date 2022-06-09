use std::any::Any;

pub trait BlockDevice: Send + Sync + Any {
    // TODO: read, write 要返回结果 Result
    // read_block中, 如果 block 长度大于 buf, 必须确保 buf 顺利读到 block 前 n 个的
    //数据, 不会被覆盖或者读取失败, 错误在Result中返回处理
    fn read_block(&self, block_id: usize, buf: &mut [u8]);
    fn write_block(&self, block_id: usize, buf: &[u8]);
}

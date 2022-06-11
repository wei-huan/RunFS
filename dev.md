# MYFAT32 设计思路

[TOC]

MYFAT32 是否该支持主引导分区MBR？
应该是不支持的，暂时没有必要，SD 卡第一个扇区就应该是 DBR，所以到时候要重刷系统

cluster_cache 的实现有问题，簇号和块号不是等比对应的。要转换

根据局部性原理真的有必要找将 FAT 表映射成 BitMap 吗
当然映射成 BitMap 好找空簇，也好调整
从缓冲区回写 SDCard 应该要在文件关闭时候
BitMap应该要 Mutex 全局性结构体，而且必须为 Arc，要在线程间安全共享

Cluster和Sector结构体大小其实是不确定的,所以问题很大,目前的实现不能运行后读取DBR改变大小.

目前实现是先扩后缩,稍微有一点点不爽.

今天是 2022 年 6 月 10 日
INFO_CACHE_MANAGER 和 DATA_CACHE_MANAGER 应该被做进 RunFileSystem 里面, 成员有 Arc BlockDevice 不好搞

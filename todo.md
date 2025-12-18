* ACPI support
  * handler impl
  * interrupt fix
* Memory Management
* Process & Thread
  * allocate Page to Process, thread use Page as stack memory but not Global Heap allocate
* Support Multi-CPU
* Use Lock guard Global Runtime
* Run on the real machine
  * extract the img, make the bootable device -- finished
  * boot from uefi/bios -- finished
* File System
  * ramfs
    * create




1. logger 转为更高级 如lock free，buffered 
2. 完善系统调用 brk 或 mmap 
3. 完善int 80h到syscall 
4. 关于文件系统 设置如系统、用户打开表 
5. 完善file_descripter使其即支持STDIO STDOUT又支持 具体文件系统文件 
6. 为用户端Stdout添加如行缓冲区 减少如printf println fmt的系统调用数
7. 为多核系统的准备 进行的数据结构重构 
8. 设备管理 
9. 虚拟化 字符设备 块设备等等
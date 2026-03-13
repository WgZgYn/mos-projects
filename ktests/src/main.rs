use vfs::{Dentry, FileType, OpenFlags, VfsResult, vfs_open};

fn main() -> VfsResult<()> {
    let root = mem_fs::get_memfs_root();

    let root_dentry = unsafe { Dentry::create_root(root) };

    let a = root_dentry.create("a.txt", FileType::Regular)?;
    println!("write data");
    a.inode().unwrap().write_at(0, b"hello")?;
    let mut buf = [0; 128];
    let n = a.inode().unwrap().read_at(0, &mut buf)?;
    println!("read from inode: {:?}", core::str::from_utf8(&buf[..n]));

    let home = root_dentry.create("home", FileType::Directory)?;
    let b = home.create("b.json", FileType::Regular)?;
    println!("ready to write data");
    let b_inode = b.inode().unwrap();
    b_inode.write_at(
        0,
        br#"
{
    "name": "wzy",
    "age": 18,
    "gender": "male",
}
    "#
        .trim_ascii(),
    )?;
    buf.fill(0);
    let n = b_inode.read_at(0, &mut buf)?;
    println!(
        "read from inode: {}",
        core::str::from_utf8(&buf[..n]).unwrap()
    );

    let file = vfs_open(
        root_dentry.clone(),
        root_dentry.clone(),
        "/home/b.json",
        OpenFlags::READ,
    )?;
    buf.fill(0);
    let n = file.read(&mut buf)?;
    println!(
        "read from vfs_open file: {}",
        core::str::from_utf8(&buf[..n]).unwrap()
    );
    Ok(())
}

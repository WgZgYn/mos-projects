use alloc::borrow::ToOwned;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use goblin::elf::program_header::{PF_R, PF_W, PF_X};
use goblin::elf::{Elf, ProgramHeader, Reloc, reloc};
use log::info;
use mos_abi::io::read_to_end;
use mos_abi::memory::{MemoryMapFlags, MemoryProtFlags};
use mos_fs_runtime::open;
use mos_lib::align::{align_down, align_up};
use mos_mem::virt::{BackingStore, MapFile, MemoryFlags, MemorySet};
use mos_vfs::file::File;
use vfs_common::{OpenFlags, VfsResult};
use x86_64::VirtAddr;
use x86_64::structures::paging::Page;
use x86_64::structures::paging::page::PageRange;

#[derive(Clone, Debug)]
struct Symbol {
    name: String,
    value: u64,
    size: u64,
    info: u8,
}

#[derive(Debug)]
struct Symbols {
    inner: Vec<Symbol>,
    index: BTreeMap<String, usize>,
}

impl Symbols {
    fn lookup_symbol_by_name(&self, name: &str) -> Option<Symbol> {
        self.index
            .get(name)
            .cloned()
            .and_then(|index| self.inner.get(index))
            .cloned()
    }
    fn lookup_symbol_by_index(&self, index: usize) -> Option<Symbol> {
        self.inner.get(index).cloned()
    }
}

#[derive(Debug)]
pub struct LoadedElf {
    name: String,

    base: Page,
    entry: VirtAddr,

    symbols: Symbols,

    rela: Vec<Reloc>,
    plt_rela: Vec<Reloc>,

    needed: Vec<String>,
}

impl LoadedElf {
    pub fn entry(&self) -> VirtAddr {
        self.entry + self.base.start_address().as_u64()
    }
}

// [start, end)
fn get_elf_page_range<'a>(phs: impl Iterator<Item = &'a ProgramHeader>) -> Option<PageRange> {
    let mut min = None;
    let mut max = None;

    phs.filter(|ph| ph.p_type == goblin::elf::program_header::PT_LOAD)
        .map(|ph| ph.p_vaddr + ph.p_memsz)
        .for_each(|vaddr| {
            if let Some(min) = &mut min {
                if vaddr < *min {
                    *min = vaddr;
                }
            } else {
                min = Some(vaddr);
            }
            if let Some(max) = &mut max {
                if vaddr > *max {
                    *max = vaddr;
                }
            } else {
                max = Some(vaddr);
            }
        });

    let min_vaddr = VirtAddr::new(min?);
    let max_vaddr = VirtAddr::new(max?);

    // [start, end)
    Some(PageRange {
        start: Page::containing_address(min_vaddr),
        end: Page::containing_address(max_vaddr.align_up(4096_u64)),
    })
}

fn parse_symbol(elf: &Elf) -> Result<Symbols, &'static str> {
    let mut inner = Vec::new();
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    for (i, sym) in elf.dynsyms.iter().enumerate() {
        let name = elf
            .dynstrtab
            .get_at(sym.st_name)
            .ok_or("should have name")?;
        inner.push(Symbol {
            name: name.to_owned(),
            value: sym.st_value,
            size: sym.st_size,
            info: sym.st_info,
        });
        index.insert(name.to_owned(), i);
    }
    Ok(Symbols { inner, index })
}

pub fn flags_elf_to_prot(elf_flags: u32) -> MemoryProtFlags {
    let mut flags = MemoryProtFlags::empty();
    if elf_flags & PF_R != 0 {
        flags.insert(MemoryProtFlags::READ);
    }
    if elf_flags & PF_W != 0 {
        flags.insert(MemoryProtFlags::WRITE);
    }
    if elf_flags & PF_X != 0 {
        flags.insert(MemoryProtFlags::EXEC);
    }
    flags
}

fn load_elf_with_lib_loader<'a>(
    mem: &MemorySet,
    elf_file: &File,
    is_fixed: bool,
    loader: &mut LibraryLoader,
) -> Result<LoadedElf, &'static str> {
    let elf_bs = read_to_end(elf_file).map_err(|e| e.as_str())?;
    let elf = Elf::parse(&elf_bs).map_err(|_| "ELF parse error")?;

    let page_range = get_elf_page_range(elf.program_headers.iter()).ok_or("should have PT_LOAD")?;
    let pages = page_range.len();

    let page_bias = if is_fixed {
        if !mem.is_free_area(page_range.into()) {
            return Err("no free area");
        }
        0
    } else {
        mem.find_free_area(pages)
            .ok_or("no free area")?
            .start
            .start_address()
            .as_u64()
            >> 12
    };

    for ph in elf.program_headers.iter() {
        if ph.p_type != goblin::elf::program_header::PT_LOAD {
            continue;
        }

        let fstart_page_i = Page::containing_address(VirtAddr::new(ph.p_vaddr));
        let fend_page_e =
            Page::containing_address(VirtAddr::new(align_up(ph.p_vaddr + ph.p_filesz, 4096)));
        let mend_page_e =
            Page::containing_address(VirtAddr::new(align_up(ph.p_vaddr + ph.p_memsz, 4096)));

        let prot = flags_elf_to_prot(ph.p_flags);
        let flags = MemoryMapFlags::PRIVATE | MemoryMapFlags::FIXED;
        let aligned_offset = align_down(ph.p_offset, 4096);

        // make sure the address to file mapping is correct when align down to page-align
        assert_eq!(
            ph.p_offset - aligned_offset,
            ph.p_vaddr - fstart_page_i.start_address().as_u64()
        );

        // mmap with file backing
        let file_mapping_pages = fend_page_e - fstart_page_i;
        let bss_mapping_pages = mend_page_e - fend_page_e;

        mem.mmap(
            Some(fstart_page_i + page_bias),
            file_mapping_pages,
            MemoryFlags::new(prot, flags),
            BackingStore::File(MapFile::new(elf_file.inode().clone(), aligned_offset)),
        )?;

        // mmap the bss with anonymous backing
        if mend_page_e > fend_page_e {
            mem.mmap(
                Some(fend_page_e + page_bias),
                bss_mapping_pages,
                MemoryFlags::new(
                    MemoryProtFlags::READ_WRITE,
                    flags | MemoryMapFlags::ANONYMOUS,
                ),
                BackingStore::Anonymous,
            )?;
        }
    }

    let symbols = parse_symbol(&elf)?;

    let mut elf_info = LoadedElf {
        name: elf_file.dentry().name(),
        base: Page::containing_address(VirtAddr::new(page_bias << 12)),
        entry: VirtAddr::new((page_bias << 12) + elf.entry),
        symbols,
        rela: elf.dynrelas.to_vec(),
        plt_rela: elf.pltrelocs.to_vec(),
        needed: vec![],
    };

    let dynamic = elf
        .program_headers
        .iter()
        .any(|ph| ph.p_type == goblin::elf::program_header::PT_DYNAMIC);
    if !dynamic {
        info!("[LINKER] LOAD static-linked ELF");
    } else {
        info!("[LINKER] LOAD dynamic-linked ELF");
        let _ = &elf.dynamic.ok_or("no dynamic")?;
        for lib in elf.libraries {
            info!("[LINKER] need lib: {}", lib);
            loader.load_with_mem(mem, lib)?;
            elf_info.needed.push(lib.to_owned());
        }
    }
    apply_reloc(&elf_info, loader)?;
    info!("[LINKER] finished loading {}", elf_info.name);
    Ok(elf_info)
}

pub fn load_elf<'a>(
    mem: &MemorySet,
    elf_file: &File,
    is_fixed: bool,
) -> Result<(LoadedElf, LibraryLoader), &'static str> {
    let mut loader = LibraryLoader {
        loaded: Default::default(),
    };
    let elf = load_elf_with_lib_loader(mem, elf_file, is_fixed, &mut loader)?;
    Ok((elf, loader))
}

fn apply_reloc(elf: &LoadedElf, loader: &LibraryLoader) -> Result<(), &'static str> {
    for reloc in elf.rela.iter().chain(elf.plt_rela.iter()) {
        let symbol = elf
            .symbols
            .lookup_symbol_by_index(reloc.r_sym)
            .ok_or("symbol not found")?;
        let name = &symbol.name;
        info!("[LINKER] try to fix {}", name);

        for lib in &elf.needed {
            // this will cause #PF to load the data, then write the fixed addr
            match reloc.r_type {
                reloc::R_X86_64_RELATIVE => {
                    info!("R_X86_64_RELATIVE");
                    unsafe {
                        (elf.base.start_address() + reloc.r_offset)
                            .as_mut_ptr::<u64>()
                            .write(
                                elf.base.start_address().as_u64()
                                    + reloc
                                        .r_addend
                                        .ok_or("R_X86_64_RELATIVE should exist r_addend")?
                                        as u64,
                            );
                    }
                }
                reloc::R_X86_64_GLOB_DAT | reloc::R_X86_64_JUMP_SLOT => {
                    info!("R_X86_64_GLOB_DAT / R_X86_64_JUMP_SLOT");
                    let lib = loader.get(lib).ok_or("lib not loaded")?;
                    let symbol = lib
                        .symbols
                        .lookup_symbol_by_name(name)
                        .ok_or("failed to lookup symbol")?;
                    let addr = lib.base.start_address() + symbol.value;
                    unsafe {
                        (elf.base.start_address() + reloc.r_offset)
                            .as_mut_ptr::<u64>()
                            .write(addr.as_u64());
                    }
                }
                _ => panic!("not supported {}", reloc.r_type),
            }
        }
    }

    Ok(())
}

pub struct LibraryLoader {
    loaded: BTreeMap<String, LoadedElf>,
}

impl LibraryLoader {
    fn load_with_mem(&mut self, mem: &MemorySet, name: &str) -> Result<(), &'static str> {
        if self.loaded.contains_key(name) {
            Err("already loaded")
        } else {
            let file = open_library(name).map_err(|e| e.as_str())?;
            let elf = load_elf_with_lib_loader(mem, &file, false, self)?;
            self.load(name.to_owned(), elf);
            Ok(())
        }
    }
    fn load(&mut self, name: String, elf: LoadedElf) {
        self.loaded.insert(name, elf);
    }
    fn get(&self, name: &str) -> Option<&LoadedElf> {
        self.loaded.get(name)
    }
}

fn open_library(name: &str) -> VfsResult<File> {
    // TODO:
    open(name, OpenFlags::READ)
}

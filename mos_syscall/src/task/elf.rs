use mos_abi::error::{KError, KResult};
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

    /// Program-header table: virtual address + entry size + entry count.
    /// Needed to fill AT_PHDR/AT_PHENT/AT_PHNUM (static musl uses AT_PHDR
    /// to locate its TLS initialization image).
    phdr: VirtAddr,
    phent: usize,
    phnum: usize,

    symbols: Symbols,

    rela: Vec<Reloc>,
    plt_rela: Vec<Reloc>,

    needed: Vec<String>,
}

impl LoadedElf {
    pub fn entry(&self) -> VirtAddr {
        self.entry + self.base.start_address().as_u64()
    }

    pub fn phdr_info(&self) -> (u64, usize, usize) {
        (self.phdr.as_u64(), self.phent, self.phnum)
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

fn parse_symbol(elf: &Elf) -> KResult<Symbols> {
    let mut inner = Vec::new();
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    for (i, sym) in elf.dynsyms.iter().enumerate() {
        let name = elf
            .dynstrtab
            .get_at(sym.st_name)
            .ok_or(KError::InvalidArg)?;
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
) -> KResult<LoadedElf> {
    let elf_bs = read_to_end(elf_file).map_err(|e| e.as_str())?;
    let elf = Elf::parse(&elf_bs).map_err(|_| "ELF parse error")?;

    let page_range = get_elf_page_range(elf.program_headers.iter()).ok_or(KError::InvalidArg)?;
    let pages = page_range.len();

    let page_bias = if is_fixed {
        if !mem.is_free_area(page_range.into()) {
            return Err(KError::NotMapped);
        }
        0
    } else {
        mem.find_free_area(pages)
            .ok_or(KError::NotMapped)?
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

        // Linux padzero: zero-fill the tail of the last file page beyond
        // p_filesz, otherwise .bss-like content would read raw file data.
        // Only for writable segments (text segments stay read-only).
        if prot.contains(MemoryProtFlags::WRITE) {
            let filesz_end = ph.p_vaddr + ph.p_filesz;
            let file_pages_end = fend_page_e.start_address().as_u64();
            if filesz_end < file_pages_end {
                let tail_addr = (page_bias << 12) + filesz_end;
                let tail_len = (file_pages_end - filesz_end) as usize;
                unsafe { core::ptr::write_bytes(tail_addr as *mut u8, 0, tail_len) };
            }
        }

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

    // e_phoff is a FILE offset: the runtime address of the phdr table is
    // load_addr_of_segment + (e_phoff - segment_file_offset). With is_fixed
    // the segment maps at its natural p_vaddr (page_bias == 0).
    let e_phoff = elf.header.e_phoff;
    let phdr = elf
        .program_headers
        .iter()
        .find(|ph| {
            ph.p_type == goblin::elf::program_header::PT_LOAD
                && ph.p_offset <= e_phoff
                && e_phoff < ph.p_offset + ph.p_filesz
        })
        .or_else(|| {
            elf.program_headers
                .iter()
                .find(|ph| ph.p_type == goblin::elf::program_header::PT_LOAD)
        })
        .map(|ph| (page_bias << 12) + ph.p_vaddr + (e_phoff - ph.p_offset))
        .unwrap_or(page_bias << 12);

    let mut elf_info = LoadedElf {
        name: elf_file.dentry().name(),
        base: Page::containing_address(VirtAddr::new(page_bias << 12)),
        entry: VirtAddr::new((page_bias << 12) + elf.entry),
        phdr: VirtAddr::new(phdr),
        phent: elf.header.e_phentsize as usize,
        phnum: elf.header.e_phnum as usize,
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
        let _ = &elf.dynamic.ok_or(KError::InvalidArg)?;
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
) -> KResult<(LoadedElf, LibraryLoader)> {
    let mut loader = LibraryLoader {
        loaded: Default::default(),
    };
    let elf = load_elf_with_lib_loader(mem, elf_file, is_fixed, &mut loader)?;
    Ok((elf, loader))
}

fn apply_reloc(elf: &LoadedElf, loader: &LibraryLoader) -> KResult<()> {
    for reloc in elf.rela.iter().chain(elf.plt_rela.iter()) {
        let symbol = elf
            .symbols
            .lookup_symbol_by_index(reloc.r_sym)
            .ok_or(KError::InvalidArg)?;
        let name = &symbol.name;
        if !name.is_empty() {
            info!("[LINKER] try to fix {}", name);
        }

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
                                        .ok_or(KError::InvalidArg)?
                                        as u64,
                            );
                    }
                }
                reloc::R_X86_64_GLOB_DAT | reloc::R_X86_64_JUMP_SLOT => {
                    info!("R_X86_64_GLOB_DAT / R_X86_64_JUMP_SLOT");
                    let lib = loader.get(lib).ok_or(KError::InvalidArg)?;
                    let symbol = lib
                        .symbols
                        .lookup_symbol_by_name(name)
                        .ok_or(KError::InvalidArg)?;
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
    fn load_with_mem(&mut self, mem: &MemorySet, name: &str) -> KResult<()> {
        if self.loaded.contains_key(name) {
            Err(KError::AlreadyMapped)
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

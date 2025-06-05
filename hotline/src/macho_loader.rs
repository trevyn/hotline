use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::ptr;
use std::slice;
use crate::tlv_support::{TLVManager, find_tlv_sections};

// constants for dlsym
#[cfg(target_os = "macos")]
const RTLD_DEFAULT: *mut libc::c_void = -2isize as *mut libc::c_void;

#[repr(C)]
struct MachHeader64 {
    magic: u32,
    cputype: i32,
    cpusubtype: i32,
    filetype: u32,
    ncmds: u32,
    sizeofcmds: u32,
    flags: u32,
    reserved: u32,
}

#[repr(C)]
struct LoadCommand {
    cmd: u32,
    cmdsize: u32,
}

#[repr(C)]
struct SegmentCommand64 {
    cmd: u32,
    cmdsize: u32,
    segname: [u8; 16],
    vmaddr: u64,
    vmsize: u64,
    fileoff: u64,
    filesize: u64,
    maxprot: i32,
    initprot: i32,
    nsects: u32,
    flags: u32,
}

#[repr(C)]
struct Section64 {
    sectname: [u8; 16],
    segname: [u8; 16],
    addr: u64,
    size: u64,
    offset: u32,
    align: u32,
    reloff: u32,
    nreloc: u32,
    flags: u32,
    reserved1: u32,
    reserved2: u32,
    reserved3: u32,
}

#[repr(C)]
struct DylibCommand {
    cmd: u32,
    cmdsize: u32,
    dylib: Dylib,
}

#[repr(C)]
struct Dylib {
    name: u32, // offset from start of this command
    timestamp: u32,
    current_version: u32,
    compatibility_version: u32,
}

#[repr(C)]
struct SymtabCommand {
    cmd: u32,
    cmdsize: u32,
    symoff: u32,
    nsyms: u32,
    stroff: u32,
    strsize: u32,
}

#[repr(C)]
struct DysymtabCommand {
    cmd: u32,
    cmdsize: u32,
    ilocalsym: u32,
    nlocalsym: u32,
    iextdefsym: u32,
    nextdefsym: u32,
    iundefsym: u32,
    nundefsym: u32,
    tocoff: u32,
    ntoc: u32,
    modtaboff: u32,
    nmodtab: u32,
    extrefsymoff: u32,
    nextrefsyms: u32,
    indirectsymoff: u32,
    nindirectsyms: u32,
    extreloff: u32,
    nextrel: u32,
    locreloff: u32,
    nlocrel: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Nlist64 {
    n_strx: u32,
    n_type: u8,
    n_sect: u8,
    n_desc: u16,
    n_value: u64,
}

// mach-o constants
const MH_MAGIC_64: u32 = 0xfeedfacf;
const MH_BUNDLE: u32 = 0x8;
const MH_DYLIB: u32 = 0x6;

const LC_SEGMENT_64: u32 = 0x19;
const LC_SYMTAB: u32 = 0x2;
const LC_DYSYMTAB: u32 = 0xb;
const LC_LOAD_DYLIB: u32 = 0xc;
const LC_ID_DYLIB: u32 = 0xd;
const LC_LOAD_WEAK_DYLIB: u32 = 0x18 | 0x80000000;
const LC_REEXPORT_DYLIB: u32 = 0x1f | 0x80000000;
const LC_LAZY_LOAD_DYLIB: u32 = 0x20;
const LC_DYLD_INFO_ONLY: u32 = 0x22 | 0x80000000;
const LC_DYLD_CHAINED_FIXUPS: u32 = 0x34 | 0x80000000;
const LC_DYLD_EXPORTS_TRIE: u32 = 0x33 | 0x80000000;

// other load commands we're seeing
const LC_UUID: u32 = 0x1b;
const LC_BUILD_VERSION: u32 = 0x32;
const LC_SOURCE_VERSION: u32 = 0x2a;
const LC_FUNCTION_STARTS: u32 = 0x26;
const LC_DATA_IN_CODE: u32 = 0x29;
const LC_CODE_SIGNATURE: u32 = 0x1d;

// dyld info structures
#[repr(C)]
struct DyldInfoCommand {
    cmd: u32,
    cmdsize: u32,
    rebase_off: u32,     // file offset to rebase info
    rebase_size: u32,    // size of rebase info
    bind_off: u32,       // file offset to binding info
    bind_size: u32,      // size of binding info
    weak_bind_off: u32,  // file offset to weak binding info
    weak_bind_size: u32, // size of weak binding info
    lazy_bind_off: u32,  // file offset to lazy binding info
    lazy_bind_size: u32, // size of lazy binding info
    export_off: u32,     // file offset to export info
    export_size: u32,    // size of export info
}

// chained fixups structures
#[repr(C)]
struct LinkeditDataCommand {
    cmd: u32,
    cmdsize: u32,
    dataoff: u32,  // file offset of data
    datasize: u32, // file size of data
}

#[repr(C)]
struct DyldChainedFixupsHeader {
    fixups_version: u32,  // 0
    starts_offset: u32,   // offset of chain starts in this blob
    imports_offset: u32,  // offset of imports table
    symbols_offset: u32,  // offset of symbol strings
    imports_count: u32,   // number of imports
    imports_format: u32,  // format of imports
    symbols_format: u32,  // format of symbol strings
}

#[repr(C)]
struct DyldChainedStartsInImage {
    seg_count: u32,
    // followed by seg_count seg_info_offset[seg_count] entries
}

#[repr(C)]
struct DyldChainedStartsInSegment {
    size: u32,              // size of this
    page_size: u16,         // 0x1000 or 0x4000
    pointer_format: u16,    // DYLD_CHAINED_PTR_*
    segment_offset: u64,    // offset in memory of this segment
    max_valid_pointer: u32, // for 32-bit OS
    page_count: u16,        // count of pages 
    // followed by page_start[] entries
}

// pointer formats
const _DYLD_CHAINED_PTR_64: u16 = 2;
const _DYLD_CHAINED_PTR_64_OFFSET: u16 = 6;
const _DYLD_CHAINED_PTR_ARM64E: u16 = 1;
const _DYLD_CHAINED_PTR_ARM64E_USERLAND: u16 = 5;

const VM_PROT_READ: i32 = 0x01;
const VM_PROT_WRITE: i32 = 0x02;
const VM_PROT_EXECUTE: i32 = 0x04;

// section types
const S_SYMBOL_STUBS: u32 = 0x8;
const S_LAZY_SYMBOL_POINTERS: u32 = 0x7;
const S_THREAD_LOCAL_VARIABLES: u32 = 0x13;
const S_MOD_INIT_FUNC_POINTERS: u32 = 0x9;

pub struct MachoLoader {
    base_addr: Option<std::ptr::NonNull<u8>>,
    base_vmaddr: u64,
    total_size: usize,
    file_data: Vec<u8>,
    segments: Vec<SegmentInfo>,
    lazy_ptr_sections: Vec<LazyPointerSection>,
    symbols: HashMap<String, u64>,
    imports: Vec<String>,
    slide: i64,  // difference between where we loaded vs where linked
    dysymtab: Option<DysymtabInfo>,
    indirect_symbols: Option<Vec<u32>>,
    symtab_symbols: Option<(usize, usize)>,
    symtab_strings: Option<(usize, usize)>,
    tlv_manager: TLVManager,
    lib_path: Option<String>,
}

// SAFETY: MachoLoader owns the memory pointed to by base_addr and ensures
// exclusive access to it. The memory is never shared between threads.
unsafe impl Send for MachoLoader {}
unsafe impl Sync for MachoLoader {}

struct SegmentInfo {
    _name: String,
    vmaddr: u64,
}

struct LazyPointerSection {
    vmaddr: u64,
    size: u64,
    indirect_offset: usize,
}

struct DysymtabInfo {
    indirectsymoff: u32,
    nindirectsyms: u32,
}

impl MachoLoader {
    pub fn new() -> Self {
        Self {
            base_addr: None,
            base_vmaddr: 0,
            total_size: 0,
            file_data: Vec::new(),
            segments: Vec::new(),
            lazy_ptr_sections: Vec::new(),
            symbols: HashMap::new(),
            imports: Vec::new(),
            slide: 0,
            dysymtab: None,
            indirect_symbols: None,
            symtab_symbols: None,
            symtab_strings: None,
            tlv_manager: TLVManager::new(),
            lib_path: None,
        }
    }

    pub unsafe fn load(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        // store the library path
        self.lib_path = Some(path.to_string());
        
        // read the entire file
        let mut file = File::open(path)?;
        let file_size = file.metadata()?.len() as usize;
        self.file_data.clear();
        self.file_data.resize(file_size, 0);
        file.read_exact(&mut self.file_data)?;
        
        // parse mach-o header
        let header = unsafe { &*(self.file_data.as_ptr() as *const MachHeader64) };
        if header.magic != MH_MAGIC_64 {
            return Err("not a 64-bit mach-o file".into());
        }
        
        if header.filetype != MH_BUNDLE && header.filetype != MH_DYLIB {
            return Err("not a bundle or dylib".into());
        }
        
        // find lowest vmaddr to determine base address
        self.base_vmaddr = unsafe { self.find_base_vmaddr()? };
        
        // allocate memory for entire image
        let total_size = unsafe { self.calculate_total_size()? };
        self.total_size = total_size as usize;
        let addr = unsafe { self.allocate_memory(self.base_vmaddr, total_size)? };
        self.base_addr = std::ptr::NonNull::new(addr);
        
        // calculate slide - difference between where we loaded vs where linked
        self.slide = addr as i64 - self.base_vmaddr as i64;
        
        // Allocated memory
        
        // process all load commands
        unsafe { self.process_load_commands()? };
        
        // process indirect symbols after load commands
        unsafe { self.process_indirect_symbols()? };
        
        // bind lazy pointers using indirect symbols
        unsafe { self.bind_lazy_pointers()? };
        
        // resolve symbols and apply relocations
        unsafe { self.resolve_symbols()? };
        
        // set up thread-local variables if present
        if let Some(base) = self.base_addr {
            let tlv_sections = unsafe { find_tlv_sections(base.as_ptr(), &self.file_data)? };
            if !tlv_sections.is_empty() {
                // Setting up thread-local variables
                unsafe { self.tlv_manager.setup_tlvs(
                    self.lib_path.as_ref().unwrap(),
                    base.as_ptr(),
                    self.slide,
                    tlv_sections
                )? };
            }
        }
        
        // run module initializers
        // unsafe { self.run_initializers()? };
        
        Ok(())
    }
    
    unsafe fn find_base_vmaddr(&self) -> Result<u64, Box<dyn std::error::Error>> {
        let header = unsafe { &*(self.file_data.as_ptr() as *const MachHeader64) };
        let mut cmd_ptr = unsafe { self.file_data.as_ptr().add(std::mem::size_of::<MachHeader64>()) };
        let mut min_addr = u64::MAX;
        
        for _ in 0..header.ncmds {
            let cmd = unsafe { &*(cmd_ptr as *const LoadCommand) };
            
            if cmd.cmd == LC_SEGMENT_64 {
                let segment = unsafe { &*(cmd_ptr as *const SegmentCommand64) };
                if segment.vmaddr < min_addr && segment.filesize > 0 {
                    min_addr = segment.vmaddr;
                }
            }
            
            cmd_ptr = unsafe { cmd_ptr.add(cmd.cmdsize as usize) };
        }
        
        Ok(min_addr)
    }
    
    unsafe fn calculate_total_size(&self) -> Result<u64, Box<dyn std::error::Error>> {
        let header = unsafe { &*(self.file_data.as_ptr() as *const MachHeader64) };
        let mut cmd_ptr = unsafe { self.file_data.as_ptr().add(std::mem::size_of::<MachHeader64>()) };
        let mut max_addr = 0u64;
        
        for _ in 0..header.ncmds {
            let cmd = unsafe { &*(cmd_ptr as *const LoadCommand) };
            
            if cmd.cmd == LC_SEGMENT_64 {
                let segment = unsafe { &*(cmd_ptr as *const SegmentCommand64) };
                let end = segment.vmaddr + segment.vmsize;
                if end > max_addr {
                    max_addr = end;
                }
            }
            
            cmd_ptr = unsafe { cmd_ptr.add(cmd.cmdsize as usize) };
        }
        
        Ok(max_addr)
    }
    
    unsafe fn allocate_memory(&self, _base_addr: u64, size: u64) -> Result<*mut u8, Box<dyn std::error::Error>> {
        // allocate anywhere - we'll adjust addresses during loading
        let addr = unsafe {
            libc::mmap(
                ptr::null_mut(),
                size as usize,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };
        
        if addr == libc::MAP_FAILED {
            return Err(format!("mmap failed: {}", std::io::Error::last_os_error()).into());
        }
        
        Ok(addr as *mut u8)
    }
    
    unsafe fn process_load_commands(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let header = unsafe { &*(self.file_data.as_ptr() as *const MachHeader64) };
        let mut cmd_ptr = unsafe { self.file_data.as_ptr().add(std::mem::size_of::<MachHeader64>()) };
        
        // Processing load commands
        
        for _i in 0..header.ncmds {
            let cmd = unsafe { &*(cmd_ptr as *const LoadCommand) };

            match cmd.cmd {
                LC_LOAD_DYLIB | LC_LOAD_WEAK_DYLIB | LC_REEXPORT_DYLIB | LC_LAZY_LOAD_DYLIB => {
                    unsafe { self.process_dylib(cmd_ptr as *const DylibCommand)? };
                }
                LC_SEGMENT_64 => unsafe { self.process_segment(cmd_ptr as *const SegmentCommand64)? },
                LC_SYMTAB => unsafe { self.process_symtab(cmd_ptr as *const SymtabCommand)? },
                LC_DYSYMTAB => {
                    // Processing LC_DYSYMTAB
                    unsafe { self.process_dysymtab(cmd_ptr as *const DysymtabCommand)? };
                },
                LC_DYLD_CHAINED_FIXUPS => {
                    // Found LC_DYLD_CHAINED_FIXUPS
                    unsafe { self.process_chained_fixups(cmd_ptr as *const LinkeditDataCommand)? };
                }
                LC_DYLD_INFO_ONLY => {
                    // Found LC_DYLD_INFO_ONLY
                    unsafe { self.process_dyld_info(cmd_ptr as *const DyldInfoCommand)? };
                }
                LC_DYLD_EXPORTS_TRIE => {
                    // Found LC_DYLD_EXPORTS_TRIE
                }
                LC_ID_DYLIB => {
                    // this dylib's ID - can skip
                }
                LC_UUID | LC_BUILD_VERSION | LC_SOURCE_VERSION | 
                LC_FUNCTION_STARTS | LC_DATA_IN_CODE | LC_CODE_SIGNATURE => {
                    // these are informational - can skip
                }
                _ => {
                    // Unknown load command
                }
            }
            
            cmd_ptr = unsafe { cmd_ptr.add(cmd.cmdsize as usize) };
        }
        
        Ok(())
    }
    
    unsafe fn process_segment(&mut self, segment: *const SegmentCommand64) -> Result<(), Box<dyn std::error::Error>> {
        let segment = unsafe { &*segment };
        let segname = std::str::from_utf8(&segment.segname)?.trim_end_matches('\0');
        
        // Loading segment
        
        // copy segment data
        if segment.filesize > 0 {
            let base = self.base_addr.ok_or("base_addr not initialized")?;
            let src = unsafe { self.file_data.as_ptr().add(segment.fileoff as usize) };
            // use vmaddr as offset from the base we allocated (not as absolute address)
            let offset = (segment.vmaddr - self.base_vmaddr) as usize;
            let dst = unsafe { base.as_ptr().add(offset) };
            unsafe { ptr::copy_nonoverlapping(src, dst, segment.filesize as usize) };
        }
        
        // process sections
        let mut section_ptr = unsafe { (segment as *const SegmentCommand64).add(1) as *const Section64 };
        for _ in 0..segment.nsects {
            let section = unsafe { &*section_ptr };
            let _sectname = std::str::from_utf8(&section.sectname)?.trim_end_matches('\0');
            let section_type = section.flags & 0xff;
            
            // check for thread local variable sections
            if section_type == S_THREAD_LOCAL_VARIABLES {
                // Found __thread_vars section
                // we'll process these after all segments are loaded
            }
            
            // check for stub sections
            if section_type == S_SYMBOL_STUBS {
                let stub_size = section.reserved2; // size of each stub
                let _stub_count = if stub_size > 0 { section.size / stub_size as u64 } else { 0 };
                // Found __stubs section
            }
            
            // check for lazy symbol pointers
            if section_type == S_LAZY_SYMBOL_POINTERS {
                let _ptr_count = section.size / 8; // 64-bit pointers
                let _indirect_offset = section.reserved1 as usize; // offset into indirect symbol table

                self.lazy_ptr_sections.push(LazyPointerSection {
                    vmaddr: section.addr,
                    size: section.size,
                    indirect_offset: section.reserved1 as usize,
                });
            }
            
            // check for module initializers
            if section_type == S_MOD_INIT_FUNC_POINTERS {
                // Found __mod_init_func section
            }
            
            section_ptr = unsafe { section_ptr.add(1) };
        }
        
        // set memory protection
        let base = self.base_addr.ok_or("base_addr not initialized")?;
        let offset = (segment.vmaddr - self.base_vmaddr) as usize;
        
        // Convert Mach-O protection flags to POSIX protection flags
        let mut prot = 0;
        if segment.initprot & VM_PROT_READ != 0 {
            prot |= libc::PROT_READ;
        }
        if segment.initprot & VM_PROT_WRITE != 0 {
            prot |= libc::PROT_WRITE;
        }
        if segment.initprot & VM_PROT_EXECUTE != 0 {
            prot |= libc::PROT_EXEC;
        }
        
        // Setting protection
        
        let result = unsafe {
            libc::mprotect(
                base.as_ptr().add(offset) as *mut libc::c_void,
                segment.vmsize as usize,
                prot,
            )
        };
        
        if result != 0 {
            return Err(format!("mprotect failed: {}", std::io::Error::last_os_error()).into());
        }
        
        self.segments.push(SegmentInfo {
            _name: segname.to_string(),
            vmaddr: segment.vmaddr,
        });
        
        Ok(())
    }
    
    // TODO: Implement module initializers support if needed
    #[allow(dead_code)]
    unsafe fn run_initializers(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Running module initializers
        
        let base = self.base_addr.ok_or("base_addr not initialized")?;
        
        // iterate through all segments to find __mod_init_func sections
        let mut _init_count = 0;
        let header_ptr = base.as_ptr() as *const MachHeader64;
        let header = unsafe { &*header_ptr };
        
        let mut cmd_ptr = unsafe { (header_ptr as *const u8).add(std::mem::size_of::<MachHeader64>()) };
        
        for _ in 0..header.ncmds {
            let cmd = unsafe { &*(cmd_ptr as *const LoadCommand) };
            
            if cmd.cmd == LC_SEGMENT_64 {
                let segment = unsafe { &*(cmd_ptr as *const SegmentCommand64) };
                
                // check sections in this segment
                let mut section_ptr = unsafe { (segment as *const SegmentCommand64).add(1) as *const Section64 };
                for _ in 0..segment.nsects {
                    let section = unsafe { &*section_ptr };
                    let section_type = section.flags & 0xff;
                    
                    if section_type == S_MOD_INIT_FUNC_POINTERS {
                        // found initializer section
                        let offset = (section.addr - self.base_vmaddr) as usize;
                        let init_ptr = unsafe { base.as_ptr().add(offset) as *const *const () };
                        let init_count_in_section = section.size as usize / std::mem::size_of::<*const ()>();
                        
                        // Running initializers from section
                        
                        for i in 0..init_count_in_section {
                            let init_func = unsafe { *init_ptr.add(i) };
                            if !init_func.is_null() {
                                // Running initializer
                                // call initializer with no arguments for now
                                // real dyld passes (argc, argv, envp, apple, vars)
                                let func: extern "C" fn() = unsafe { std::mem::transmute(init_func) };
                                func();
                                _init_count += 1;
                            }
                        }
                    }
                    
                    section_ptr = unsafe { section_ptr.add(1) };
                }
            }
            
            cmd_ptr = unsafe { cmd_ptr.add(cmd.cmdsize as usize) };
        }
        
        // Ran initializers
        Ok(())
    }
    
    unsafe fn process_symtab(&mut self, symtab: *const SymtabCommand) -> Result<(), Box<dyn std::error::Error>> {
        let symtab = unsafe { &*symtab };
        let symbols = unsafe {
            slice::from_raw_parts(
                self.file_data.as_ptr().add(symtab.symoff as usize) as *const Nlist64,
                symtab.nsyms as usize,
            )
        };

        let strings = unsafe { self.file_data.as_ptr().add(symtab.stroff as usize) };

        // store offsets for indirect symbol resolution
        self.symtab_symbols = Some((symtab.symoff as usize, symtab.nsyms as usize));
        let string_size = self.file_data.len() - symtab.stroff as usize;
        self.symtab_strings = Some((symtab.stroff as usize, string_size));
        
        let mut _exported_count = 0;
        for symbol in symbols {
            if symbol.n_strx > 0 {
                let name_ptr = unsafe { strings.add(symbol.n_strx as usize) };
                let name = unsafe { std::ffi::CStr::from_ptr(name_ptr as *const i8) }.to_string_lossy().into_owned();
                
                // Only store external symbols (exported)
                if symbol.n_value > 0 && (symbol.n_type & 0x01) != 0 {
                    self.symbols.insert(name.clone(), symbol.n_value);
                    _exported_count += 1;
                    
                    // Debug: print init symbols
                    // if name.contains("__init__registry__") {
                    //     println!("  [*] Found init symbol: {} at n_value: 0x{:x} (type: 0x{:x}, sect: {})", 
                    //         name, symbol.n_value, symbol.n_type, symbol.n_sect);
                    // }
                }
            }
        }
        
        // println!("[*] Loaded {} exported symbols (from {} total)", exported_count, symbols.len());
        Ok(())
    }
    
    unsafe fn process_dysymtab(&mut self, dysymtab: *const DysymtabCommand) -> Result<(), Box<dyn std::error::Error>> {
        let dysymtab = unsafe { &*dysymtab };
        // Processing LC_DYSYMTAB
        
        // store dysymtab info for later use
        self.dysymtab = Some(DysymtabInfo {
            indirectsymoff: dysymtab.indirectsymoff,
            nindirectsyms: dysymtab.nindirectsyms,
        });
        
        Ok(())
    }
    
    unsafe fn process_indirect_symbols(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(dysymtab) = &self.dysymtab {
            if dysymtab.nindirectsyms > 0 {
                // read indirect symbol table
                let indirect_syms = unsafe {
                    slice::from_raw_parts(
                        self.file_data.as_ptr().add(dysymtab.indirectsymoff as usize) as *const u32,
                        dysymtab.nindirectsyms as usize
                    )
                };
                
                // Processing indirect symbol table
                
                // store for later use when processing stubs
                self.indirect_symbols = Some(indirect_syms.to_vec());
            }
        }
        Ok(())
    }
    
    unsafe fn bind_lazy_pointers(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let Some(indirect_symbols) = &self.indirect_symbols else { return Ok(()); };
        let Some(symtab_symbols) = &self.symtab_symbols else { return Ok(()); };
        let Some(symtab_strings) = &self.symtab_strings else { return Ok(()); };

        let base = self.base_addr.ok_or("base_addr not initialized")?;

        let (sym_off, sym_count) = *symtab_symbols;
        let (str_off, str_size) = *symtab_strings;

        let symtab_symbols = unsafe {
            slice::from_raw_parts(
                self.file_data.as_ptr().add(sym_off) as *const Nlist64,
                sym_count,
            )
        };
        let strings_ptr = unsafe { self.file_data.as_ptr().add(str_off) };

        for section in &self.lazy_ptr_sections {
            let ptr_count = section.size / 8;
            let indirect_offset = section.indirect_offset;
            let offset = (section.vmaddr - self.base_vmaddr) as usize;
            let la_ptr_base = unsafe { base.as_ptr().add(offset) as *mut u64 };

            for i in 0..ptr_count {
                let indirect_index = indirect_offset + i as usize;
                if indirect_index >= indirect_symbols.len() {
                    continue;
                }

                let symbol_index = indirect_symbols[indirect_index] as usize;
                const INDIRECT_SYMBOL_LOCAL: u32 = 0x80000000;
                const INDIRECT_SYMBOL_ABS: u32 = 0x40000000;
                if symbol_index == INDIRECT_SYMBOL_LOCAL as usize
                    || symbol_index == INDIRECT_SYMBOL_ABS as usize
                    || symbol_index == (INDIRECT_SYMBOL_LOCAL | INDIRECT_SYMBOL_ABS) as usize
                {
                    continue;
                }

                if symbol_index < symtab_symbols.len() {
                    let symbol = &symtab_symbols[symbol_index];
                    if symbol.n_strx > 0 && (symbol.n_strx as usize) < str_size {
                        let name_ptr = unsafe { strings_ptr.add(symbol.n_strx as usize) };
                        let name = unsafe { std::ffi::CStr::from_ptr(name_ptr as *const i8) }.to_string_lossy();
                        if let Ok(addr) = self.resolve_external_symbol(&name, "/usr/lib/libSystem.B.dylib") {
                            unsafe { *la_ptr_base.add(i as usize) = addr as u64 };
                        }
                    }
                }
            }
        }

        Ok(())
    }
    
    unsafe fn process_dylib(&mut self, dylib_cmd: *const DylibCommand) -> Result<(), Box<dyn std::error::Error>> {
        let name_offset = unsafe { (*dylib_cmd).dylib.name } as usize;
        let name_ptr = unsafe { (dylib_cmd as *const u8).add(name_offset) };
        let name = unsafe { std::ffi::CStr::from_ptr(name_ptr as *const i8) }.to_string_lossy().into_owned();
        
        // Import dylib
        self.imports.push(name);
        
        Ok(())
    }
    
    unsafe fn process_dyld_info(&mut self, cmd: *const DyldInfoCommand) -> Result<(), Box<dyn std::error::Error>> {
        let cmd = unsafe { &*cmd };
        // Processing LC_DYLD_INFO_ONLY
        
        if cmd.rebase_size > 0 {
            // rebase info
            unsafe { self.process_rebase_info(cmd.rebase_off, cmd.rebase_size)? };
        }
        if cmd.bind_size > 0 {
            // bind info
            unsafe { self.process_bind_info(cmd.bind_off, cmd.bind_size, false)? };
        }
        if cmd.weak_bind_size > 0 {
            // weak bind info
        }
        if cmd.lazy_bind_size > 0 {
            // lazy bind info
            // process lazy binding eagerly to avoid needing dyld_stub_binder
            unsafe { self.process_bind_info(cmd.lazy_bind_off, cmd.lazy_bind_size, true)? };
        }
        if cmd.export_size > 0 {
            // export info
        }
        
        Ok(())
    }
    
    unsafe fn process_chained_fixups(&mut self, cmd: *const LinkeditDataCommand) -> Result<(), Box<dyn std::error::Error>> {
        let cmd = unsafe { &*cmd };
        // Processing chained fixups
        
        // get the fixups header
        let header_ptr = unsafe { self.file_data.as_ptr().add(cmd.dataoff as usize) as *const DyldChainedFixupsHeader };
        let header = unsafe { &*header_ptr };
        
        // fixups header info
        
        // get chain starts
        let starts_ptr = unsafe { (header_ptr as *const u8).add(header.starts_offset as usize) as *const DyldChainedStartsInImage };
        let starts = unsafe { &*starts_ptr };
        
        // segment count
        
        // get segment offsets array
        let seg_offsets = unsafe {
            slice::from_raw_parts(
                (starts_ptr as *const u8).add(std::mem::size_of::<DyldChainedStartsInImage>()) as *const u32,
                starts.seg_count as usize
            )
        };
        
        // process each segment's fixup chains
        for (_i, &offset) in seg_offsets.iter().enumerate() {
            if offset == 0 {
                // segment has no fixups
                continue;
            }
            
            let seg_starts_ptr = unsafe { 
                (starts_ptr as *const u8).add(offset as usize) as *const DyldChainedStartsInSegment 
            };
            let _seg_starts = unsafe { &*seg_starts_ptr };
            
            // segment fixups info
        }
        
        // TODO: actually process the fixup chains and apply relocations
        // TODO: apply chained fixups
        
        Ok(())
    }
    
    unsafe fn process_rebase_info(&mut self, offset: u32, size: u32) -> Result<(), Box<dyn std::error::Error>> {
        // rebase opcodes
        const REBASE_OPCODE_DONE: u8 = 0x00;
        const REBASE_OPCODE_SET_TYPE_IMM: u8 = 0x10;
        const REBASE_OPCODE_SET_SEGMENT_AND_OFFSET_ULEB: u8 = 0x20;
        const REBASE_OPCODE_ADD_ADDR_ULEB: u8 = 0x30;
        const REBASE_OPCODE_ADD_ADDR_IMM_SCALED: u8 = 0x40;
        const REBASE_OPCODE_DO_REBASE_IMM_TIMES: u8 = 0x50;
        const REBASE_OPCODE_DO_REBASE_ULEB_TIMES: u8 = 0x60;
        const REBASE_OPCODE_DO_REBASE_ADD_ADDR_ULEB: u8 = 0x70;
        const REBASE_OPCODE_DO_REBASE_ULEB_TIMES_SKIPPING_ULEB: u8 = 0x80;
        
        const REBASE_TYPE_POINTER: u8 = 1;
        const _REBASE_TYPE_TEXT_ABSOLUTE32: u8 = 2;
        const _REBASE_TYPE_TEXT_PCREL32: u8 = 3;
        
        if self.slide == 0 {
            // No slide, skipping rebase
            return Ok(());
        }
        
        let base = self.base_addr.ok_or("base_addr not set")?;
        let data = &self.file_data[offset as usize..(offset + size) as usize];
        let mut p = 0;
        
        let mut segment_index = 0;
        let mut segment_offset = 0u64;
        let mut _rebase_type = REBASE_TYPE_POINTER;
        let mut _rebase_count = 0;
        
        // Processing rebase info
        
        while p < data.len() {
            let opcode = data[p];
            p += 1;
            
            let immediate = opcode & 0x0F;
            let opcode_type = opcode & 0xF0;
            
            match opcode_type {
                REBASE_OPCODE_DONE => {
                    break;
                }
                REBASE_OPCODE_SET_TYPE_IMM => {
                    _rebase_type = immediate;
                }
                REBASE_OPCODE_SET_SEGMENT_AND_OFFSET_ULEB => {
                    segment_index = immediate as usize;
                    let (offset, consumed) = self.read_uleb128(&data[p..]);
                    segment_offset = offset;
                    p += consumed;
                }
                REBASE_OPCODE_ADD_ADDR_ULEB => {
                    let (offset, consumed) = self.read_uleb128(&data[p..]);
                    segment_offset += offset;
                    p += consumed;
                }
                REBASE_OPCODE_ADD_ADDR_IMM_SCALED => {
                    segment_offset += (immediate as u64) * 8; // pointer size
                }
                REBASE_OPCODE_DO_REBASE_IMM_TIMES => {
                    for _ in 0..immediate {
                        if segment_index < self.segments.len() {
                            let seg = &self.segments[segment_index];
                            let addr = (seg.vmaddr - self.base_vmaddr) + segment_offset;
                            let ptr = unsafe { base.as_ptr().add(addr as usize) as *mut u64 };
                            let old_value = unsafe { *ptr };
                            let new_value = old_value.wrapping_add(self.slide as u64);
                            unsafe { *ptr = new_value };
                            _rebase_count += 1;
                        }
                        segment_offset += 8; // pointer size
                    }
                }
                REBASE_OPCODE_DO_REBASE_ULEB_TIMES => {
                    let (count, consumed) = self.read_uleb128(&data[p..]);
                    p += consumed;
                    for _ in 0..count {
                        if segment_index < self.segments.len() {
                            let seg = &self.segments[segment_index];
                            let addr = (seg.vmaddr - self.base_vmaddr) + segment_offset;
                            let ptr = unsafe { base.as_ptr().add(addr as usize) as *mut u64 };
                            let old_value = unsafe { *ptr };
                            let new_value = old_value.wrapping_add(self.slide as u64);
                            unsafe { *ptr = new_value };
                            _rebase_count += 1;
                        }
                        segment_offset += 8; // pointer size
                    }
                }
                REBASE_OPCODE_DO_REBASE_ADD_ADDR_ULEB => {
                    if segment_index < self.segments.len() {
                        let seg = &self.segments[segment_index];
                        let addr = (seg.vmaddr - self.base_vmaddr) + segment_offset;
                        let ptr = unsafe { base.as_ptr().add(addr as usize) as *mut u64 };
                        let old_value = unsafe { *ptr };
                        let new_value = old_value.wrapping_add(self.slide as u64);
                        unsafe { *ptr = new_value };
                        _rebase_count += 1;
                    }
                    let (offset, consumed) = self.read_uleb128(&data[p..]);
                    segment_offset += offset + 8; // add offset + pointer size
                    p += consumed;
                }
                REBASE_OPCODE_DO_REBASE_ULEB_TIMES_SKIPPING_ULEB => {
                    let (count, consumed) = self.read_uleb128(&data[p..]);
                    p += consumed;
                    let (skip, consumed) = self.read_uleb128(&data[p..]);
                    p += consumed;
                    
                    for _ in 0..count {
                        if segment_index < self.segments.len() {
                            let seg = &self.segments[segment_index];
                            let addr = (seg.vmaddr - self.base_vmaddr) + segment_offset;
                            let ptr = unsafe { base.as_ptr().add(addr as usize) as *mut u64 };
                            let old_value = unsafe { *ptr };
                            let new_value = old_value.wrapping_add(self.slide as u64);
                            unsafe { *ptr = new_value };
                            _rebase_count += 1;
                        }
                        segment_offset += skip + 8; // skip + pointer size
                    }
                }
                _ => {
                    return Err(format!("Unknown rebase opcode: 0x{:02x}", opcode).into());
                }
            }
        }
        
        // Applied rebases
        Ok(())
    }
    
    unsafe fn process_bind_info(&mut self, offset: u32, size: u32, _is_lazy: bool) -> Result<(), Box<dyn std::error::Error>> {
        // bind opcodes
        const BIND_OPCODE_DONE: u8 = 0x00;
        const BIND_OPCODE_SET_DYLIB_ORDINAL_IMM: u8 = 0x10;
        const BIND_OPCODE_SET_DYLIB_ORDINAL_ULEB: u8 = 0x20;
        const BIND_OPCODE_SET_DYLIB_SPECIAL_IMM: u8 = 0x30;
        const BIND_OPCODE_SET_SYMBOL_TRAILING_FLAGS_IMM: u8 = 0x40;
        const BIND_OPCODE_SET_TYPE_IMM: u8 = 0x50;
        const BIND_OPCODE_SET_ADDEND_SLEB: u8 = 0x60;
        const BIND_OPCODE_SET_SEGMENT_AND_OFFSET_ULEB: u8 = 0x70;
        const BIND_OPCODE_ADD_ADDR_ULEB: u8 = 0x80;
        const BIND_OPCODE_DO_BIND: u8 = 0x90;
        const BIND_OPCODE_DO_BIND_ADD_ADDR_ULEB: u8 = 0xA0;
        const BIND_OPCODE_DO_BIND_ADD_ADDR_IMM_SCALED: u8 = 0xB0;
        const BIND_OPCODE_DO_BIND_ULEB_TIMES_SKIPPING_ULEB: u8 = 0xC0;
        
        const BIND_TYPE_POINTER: u8 = 1;
        const _BIND_TYPE_TEXT_ABSOLUTE32: u8 = 2;
        const _BIND_TYPE_TEXT_PCREL32: u8 = 3;
        
        let data = &self.file_data[offset as usize..(offset + size) as usize];
        let mut p = 0;
        
        let mut lib_ordinal = 0;
        let mut symbol_name = String::new();
        let mut segment_index = 0;
        let mut segment_offset = 0u64;
        let mut _bind_type = BIND_TYPE_POINTER;
        let mut _addend = 0i64;
        let mut _bind_count = 0;
        
        // Processing bind info
        
        while p < data.len() {
            let opcode = data[p];
            p += 1;
            
            let immediate = opcode & 0x0F;
            let opcode_type = opcode & 0xF0;
            
            match opcode_type {
                BIND_OPCODE_DONE => {
                    break;
                }
                BIND_OPCODE_SET_DYLIB_ORDINAL_IMM => {
                    lib_ordinal = immediate as usize;
                }
                BIND_OPCODE_SET_DYLIB_ORDINAL_ULEB => {
                    let (ordinal, consumed) = self.read_uleb128(&data[p..]);
                    lib_ordinal = ordinal as usize;
                    p += consumed;
                }
                BIND_OPCODE_SET_DYLIB_SPECIAL_IMM => {
                    // special ordinals (negative)
                    lib_ordinal = 0; // TODO: handle special ordinals properly
                }
                BIND_OPCODE_SET_SYMBOL_TRAILING_FLAGS_IMM => {
                    // read null-terminated symbol name
                    symbol_name.clear();
                    while p < data.len() && data[p] != 0 {
                        symbol_name.push(data[p] as char);
                        p += 1;
                    }
                    if p < data.len() {
                        p += 1; // skip null terminator
                    }
                }
                BIND_OPCODE_SET_TYPE_IMM => {
                    _bind_type = immediate;
                }
                BIND_OPCODE_SET_ADDEND_SLEB => {
                    let (add, consumed) = self.read_sleb128(&data[p..]);
                    _addend = add;
                    p += consumed;
                }
                BIND_OPCODE_SET_SEGMENT_AND_OFFSET_ULEB => {
                    segment_index = immediate as usize;
                    let (offset, consumed) = self.read_uleb128(&data[p..]);
                    segment_offset = offset;
                    p += consumed;
                }
                BIND_OPCODE_ADD_ADDR_ULEB => {
                    let (offset, consumed) = self.read_uleb128(&data[p..]);
                    segment_offset += offset;
                    p += consumed;
                }
                BIND_OPCODE_DO_BIND => {
                    let lib_name = if lib_ordinal > 0 && lib_ordinal <= self.imports.len() {
                        &self.imports[lib_ordinal - 1]  // ordinals are 1-indexed
                    } else {
                        "unknown"
                    };
                    
                    // actually perform the bind
                    let symbol_addr = self.resolve_external_symbol(&symbol_name, lib_name)?;
                    
                    if segment_index < self.segments.len() {
                        let seg = &self.segments[segment_index];
                        let base = self.base_addr.ok_or("base_addr not set")?;
                        let addr = (seg.vmaddr - self.base_vmaddr) + segment_offset;
                        let ptr = unsafe { base.as_ptr().add(addr as usize) as *mut u64 };
                        
                        // Binding symbol
                        
                        unsafe { *ptr = symbol_addr as u64 };
                        
                        // Bound symbol
                    }
                    
                    _bind_count += 1;
                    segment_offset += 8; // pointer size
                }
                BIND_OPCODE_DO_BIND_ADD_ADDR_ULEB => {
                    let lib_name = if lib_ordinal > 0 && lib_ordinal <= self.imports.len() {
                        &self.imports[lib_ordinal - 1]  // ordinals are 1-indexed
                    } else {
                        "unknown"
                    };
                    
                    // actually perform the bind
                    let symbol_addr = self.resolve_external_symbol(&symbol_name, lib_name)?;
                    
                    if segment_index < self.segments.len() {
                        let seg = &self.segments[segment_index];
                        let base = self.base_addr.ok_or("base_addr not set")?;
                        let addr = (seg.vmaddr - self.base_vmaddr) + segment_offset;
                        let ptr = unsafe { base.as_ptr().add(addr as usize) as *mut u64 };
                        unsafe { *ptr = symbol_addr as u64 };
                        
                        // Bound symbol
                    }
                    
                    _bind_count += 1;
                    let (offset, consumed) = self.read_uleb128(&data[p..]);
                    segment_offset += offset + 8;
                    p += consumed;
                }
                BIND_OPCODE_DO_BIND_ADD_ADDR_IMM_SCALED => {
                    let lib_name = if lib_ordinal > 0 && lib_ordinal <= self.imports.len() {
                        &self.imports[lib_ordinal - 1]  // ordinals are 1-indexed
                    } else {
                        "unknown"
                    };
                    
                    // actually perform the bind
                    let symbol_addr = self.resolve_external_symbol(&symbol_name, lib_name)?;
                    
                    if segment_index < self.segments.len() {
                        let seg = &self.segments[segment_index];
                        let base = self.base_addr.ok_or("base_addr not set")?;
                        let addr = (seg.vmaddr - self.base_vmaddr) + segment_offset;
                        let ptr = unsafe { base.as_ptr().add(addr as usize) as *mut u64 };
                        unsafe { *ptr = symbol_addr as u64 };
                        
                        // Bound symbol
                    }
                    
                    _bind_count += 1;
                    segment_offset += (immediate as u64 + 1) * 8;
                }
                BIND_OPCODE_DO_BIND_ULEB_TIMES_SKIPPING_ULEB => {
                    let (count, consumed) = self.read_uleb128(&data[p..]);
                    p += consumed;
                    let (skip, consumed) = self.read_uleb128(&data[p..]);
                    p += consumed;
                    
                    for _i in 0..count {
                        let lib_name = if lib_ordinal > 0 && lib_ordinal <= self.imports.len() {
                            &self.imports[lib_ordinal - 1]  // ordinals are 1-indexed
                        } else {
                            "unknown"
                        };
                        
                        // actually perform the bind
                        let symbol_addr = self.resolve_external_symbol(&symbol_name, lib_name)?;
                        
                        if segment_index < self.segments.len() {
                            let seg = &self.segments[segment_index];
                            let base = self.base_addr.ok_or("base_addr not set")?;
                            let addr = (seg.vmaddr - self.base_vmaddr) + segment_offset;
                            let ptr = unsafe { base.as_ptr().add(addr as usize) as *mut u64 };
                            unsafe { *ptr = symbol_addr as u64 };
                            
                            // Bound symbol
                        }
                        
                        _bind_count += 1;
                        segment_offset += skip + 8;
                    }
                }
                _ => {
                    return Err(format!("Unknown bind opcode: 0x{:02x}", opcode).into());
                }
            }
        }
        
        // Processed bindings
        Ok(())
    }
    
    fn read_sleb128(&self, data: &[u8]) -> (i64, usize) {
        let mut result = 0i64;
        let mut shift = 0;
        let mut consumed = 0;
        let mut byte = 0u8;
        
        for &b in data {
            consumed += 1;
            byte = b;
            
            result |= ((byte & 0x7f) as i64) << shift;
            shift += 7;
            
            if byte & 0x80 == 0 {
                break;
            }
        }
        
        // sign extend if necessary
        if shift < 64 && (byte & 0x40) != 0 {
            result |= !0 << shift;
        }
        
        (result, consumed)
    }
    
    fn read_uleb128(&self, data: &[u8]) -> (u64, usize) {
        let mut result = 0u64;
        let mut shift = 0;
        let mut consumed = 0;
        
        for &byte in data {
            consumed += 1;
            
            result |= ((byte & 0x7f) as u64) << shift;
            shift += 7;
            
            if byte & 0x80 == 0 {
                break;
            }
        }
        
        (result, consumed)
    }
    
    fn resolve_external_symbol(&self, symbol: &str, lib_name: &str) -> Result<usize, Box<dyn std::error::Error>> {
        // Resolving symbol
        
        // for now, use dlsym to resolve symbols from system libraries
        // in a full implementation, we'd parse the export trie of the target library
        
        // special handling for common system symbols
        if lib_name.contains("libSystem") {
            // special case for __tlv_bootstrap - we don't use the system's version
            if symbol == "__tlv_bootstrap" {
                // We handle TLVs ourselves, so we don't need to resolve this
                // Skipping __tlv_bootstrap (using hotline TLV implementation)
                // Return a dummy address - it shouldn't be called since we update the thunks
                return Ok(0x1000);
            }
            
            // special case for dyld_stub_binder
            if symbol == "dyld_stub_binder" {
                // dyld_stub_binder is needed for lazy binding, but since we process
                // lazy binds eagerly, we can skip it
                // Skipping dyld_stub_binder (lazy binds processed eagerly)
                // return a dummy address - it shouldn't be called
                return Ok(0x1000);
            }
            
            // try with underscore prefix first (standard macOS symbol naming)
            let prefixed_symbol = format!("_{}", symbol);
            let c_symbol = std::ffi::CString::new(prefixed_symbol)?;
            let addr = unsafe { libc::dlsym(RTLD_DEFAULT, c_symbol.as_ptr()) };
            
            if !addr.is_null() {
                // Resolved symbol
                return Ok(addr as usize);
            }
            
            // try without prefix
            let c_symbol = std::ffi::CString::new(symbol)?;
            let addr = unsafe { libc::dlsym(RTLD_DEFAULT, c_symbol.as_ptr()) };
            
            if !addr.is_null() {
                // Resolved symbol
                return Ok(addr as usize);
            }
            
            // if symbol starts with underscore, also try without it (e.g. _CCRandomGenerateBytes -> CCRandomGenerateBytes)
            if symbol.starts_with('_') {
                let unprefixed = &symbol[1..];
                let c_symbol = std::ffi::CString::new(unprefixed)?;
                let addr = unsafe { libc::dlsym(RTLD_DEFAULT, c_symbol.as_ptr()) };
                
                if !addr.is_null() {
                    // Resolved symbol
                    return Ok(addr as usize);
                }
            }
        }
        
        Err(format!("Failed to resolve symbol {} from {}", symbol, lib_name).into())
    }
    
    unsafe fn resolve_symbols(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: implement symbol resolution and binding
        // For now, warn about unresolved imports
        if !self.imports.is_empty() {
            // println!("[!] Warning: Dynamic imports not resolved:");
            // for import in &self.imports {
            //     println!("    - {}", import);
            // }
            // println!("[!] Loaded code may crash if it calls imported functions");
        }
        Ok(())
    }
    
    pub unsafe fn get_symbol(&self, name: &str) -> Option<*const u8> {
        match self.base_addr {
            Some(base) => {
                // Try with underscore prefix first (standard macOS symbol naming)
                let prefixed_name = format!("_{}", name);
                let addr = self.symbols.get(&prefixed_name)
                    .or_else(|| self.symbols.get(name));
                
                match addr {
                    Some(&addr) => {
                        let offset = (addr - self.base_vmaddr) as usize;
                        let final_addr = unsafe { base.as_ptr().add(offset) as *const u8 };
                        Some(final_addr)
                    }
                    None => {
                        // Symbol not found in symbol table
                        None
                    }
                }
            },
            None => None,
        }
    }
    
    // removed call_function - callers should use get_symbol and transmute directly
}

impl Drop for MachoLoader {
    fn drop(&mut self) {
        if let Some(base) = self.base_addr {
            if self.total_size > 0 {
                unsafe {
                    libc::munmap(base.as_ptr() as *mut libc::c_void, self.total_size);
                }
            }
        }
    }
}
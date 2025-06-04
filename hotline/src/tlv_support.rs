use std::slice;
use std::collections::HashMap;
use libc::{pthread_key_t, pthread_key_create, pthread_getspecific, pthread_setspecific};

// Runtime TLV thunk structure for 64-bit (matches dyld's TLV_Thunkv2)
#[repr(C)]
pub struct TLVThunkv2 {
    func: *const u8,
    key: u32,
    offset: u32,
    initial_content_delta: i32,  // delta to initial content, or 0 for zero-fill
    initial_content_size: u32,
}

// TLV section info
pub struct TLVSectionInfo {
    pub thunks_addr: u64,
    pub thunks_size: u64,
    pub initial_content_addr: u64,
    pub initial_content_size: u64,
    pub all_zero_fill: bool,
}

pub struct TLVManager {
    // Map from library path to pthread key
    keys: HashMap<String, pthread_key_t>,
}

impl TLVManager {
    pub fn new() -> Self {
        Self {
            keys: HashMap::new(),
        }
    }

    // Set up TLVs for a loaded library
    pub unsafe fn setup_tlvs(
        &mut self,
        lib_path: &str,
        base_addr: *mut u8,
        _slide: i64,
        tlv_sections: Vec<TLVSectionInfo>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if tlv_sections.is_empty() {
            return Ok(());
        }

        // Setting up TLVs

        // Allocate pthread key for this library
        let mut key: pthread_key_t = 0;
        let result = unsafe { pthread_key_create(&mut key, Some(free_tlv)) };
        if result != 0 {
            return Err("Failed to create pthread key for TLVs".into());
        }
        
        self.keys.insert(lib_path.to_string(), key);

        // Use our own TLV handler
        let tlv_get_addr = hotline_tlv_get_addr as *const u8;

        // Process each TLV section
        for section in &tlv_sections {
            // thunks_addr is a VM address from the Mach-O file
            // The correct address is base_addr + (thunks_addr - base_vmaddr)
            // Since base_vmaddr is typically 0 for dylibs, this simplifies to base_addr + thunks_addr
            let thunks_ptr = unsafe { base_addr.add(section.thunks_addr as usize) };
            let thunk_count = section.thunks_size / std::mem::size_of::<TLVThunkv2>() as u64;
            

            // Initial content location (if not zero-fill)
            let initial_content_ptr = if !section.all_zero_fill && section.initial_content_size > 0 {
                Some(unsafe { base_addr.add(section.initial_content_addr as usize) })
            } else {
                None
            };

            // Update each thunk
            // First read as on-disk Thunk format (3 x 64-bit pointers)
            #[repr(C)]
            struct DiskThunk {
                func: *const u8,
                key: usize,    // 64-bit on disk
                offset: usize, // 64-bit on disk  
            }
            let disk_thunks = unsafe { slice::from_raw_parts(thunks_ptr as *const DiskThunk, thunk_count as usize) };
            
            // Now reinterpret as runtime TLVThunkv2 format for writing
            let thunks = unsafe { slice::from_raw_parts_mut(thunks_ptr as *mut TLVThunkv2, thunk_count as usize) };
            
            for (disk_thunk, thunkv2) in disk_thunks.iter().zip(thunks.iter_mut()) {
                // Since we're manually loading, we always need to set up the thunks
                // The func pointer in the file is just a placeholder that would normally be relocated
                
                // Update the thunk with our TLV info
                thunkv2.func = tlv_get_addr as *const u8;
                thunkv2.key = key as u32;
                // Use the offset from the disk format
                thunkv2.offset = disk_thunk.offset as u32;
                
                // Set up initial content info
                if let Some(content_ptr) = initial_content_ptr {
                    // Calculate delta from thunk field to initial content  
                    let delta_field_ptr = &thunkv2.initial_content_delta as *const i32 as *const u8;
                    thunkv2.initial_content_delta = unsafe { content_ptr.offset_from(delta_field_ptr) as i32 };
                    thunkv2.initial_content_size = section.initial_content_size as u32;
                } else {
                    // Zero-fill case
                    thunkv2.initial_content_delta = 0;
                    thunkv2.initial_content_size = section.initial_content_size as u32;
                }
                
            }
        }

        Ok(())
    }

    // Clean up when library is unloaded
    #[allow(dead_code)]
    pub fn cleanup(&mut self, lib_path: &str) {
        if let Some(_key) = self.keys.remove(lib_path) {
            // pthread_key_delete is not safe to call if threads might still have values
            // Let the OS clean up when the process exits
            // Would cleanup TLV key
        }
    }
}

// Our own implementation of __tlv_get_addr for ARM64
// This is called when code accesses a thread-local variable
// Input: pointer to TLVThunkv2 in x0
// Output: address of the TLV in x0
// IMPORTANT: This function must preserve all registers except x0, x16, x17
#[unsafe(no_mangle)]
#[cfg(target_arch = "aarch64")]
pub unsafe extern "C" fn hotline_tlv_get_addr(thunk: *const TLVThunkv2) -> *mut u8 {
    if thunk.is_null() {
        panic!("hotline_tlv_get_addr called with null thunk");
    }
    
    let thunk = unsafe { &*thunk };
    let key = thunk.key;
    let offset = thunk.offset;
    
    // Get the thread's TLV allocation for this key
    let allocation = unsafe { pthread_getspecific(key as pthread_key_t) as *mut u8 };
    
    if !allocation.is_null() {
        // Already allocated, return address
        return unsafe { allocation.add(offset as usize) };
    }
    
    // First use - need to allocate and initialize
    // For now, allocate based on the size in the thunk
    let size = thunk.initial_content_size as usize;
    let allocation = unsafe { libc::malloc(size) as *mut u8 };
    
    if allocation.is_null() {
        panic!("Failed to allocate TLV storage");
    }
    
    // Initialize the content
    if thunk.initial_content_delta != 0 {
        // Copy initial content
        let delta_field_ptr = &thunk.initial_content_delta as *const i32 as *const u8;
        let content_ptr = unsafe { delta_field_ptr.offset(thunk.initial_content_delta as isize) };
        unsafe { std::ptr::copy_nonoverlapping(content_ptr, allocation, size) };
    } else {
        // Zero-fill
        unsafe { std::ptr::write_bytes(allocation, 0, size) };
    }
    
    // Store the allocation for this thread
    unsafe { pthread_setspecific(key as pthread_key_t, allocation as *const libc::c_void) };
    
    // Return the address of the specific TLV
    unsafe { allocation.add(offset as usize) }
}

// Callback for pthread to free TLV storage
extern "C" fn free_tlv(ptr: *mut libc::c_void) {
    if !ptr.is_null() {
        unsafe { libc::free(ptr) };
    }
}

// Mach-O constants needed for TLV processing
const LC_SEGMENT_64: u32 = 0x19;
const S_THREAD_LOCAL_VARIABLES: u32 = 0x13;
const S_THREAD_LOCAL_REGULAR: u32 = 0x11;
const S_THREAD_LOCAL_ZEROFILL: u32 = 0x12;

// Mach-O structures needed for parsing
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

// Helper to find TLV sections in a loaded image
pub unsafe fn find_tlv_sections(
    _base_addr: *const u8,
    file_data: &[u8],
) -> Result<Vec<TLVSectionInfo>, Box<dyn std::error::Error>> {
    
    let mut tlv_sections = Vec::new();
    let mut initial_content_start: Option<u64> = None;
    let mut initial_content_size = 0u64;
    let mut all_zero_fill = true;
    
    let header = unsafe { &*(file_data.as_ptr() as *const MachHeader64) };
    let mut cmd_ptr = unsafe { file_data.as_ptr().add(std::mem::size_of::<MachHeader64>()) };
    
    // First pass: find TLV sections and initial content
    for _ in 0..header.ncmds {
        let cmd = unsafe { &*(cmd_ptr as *const LoadCommand) };
        
        if cmd.cmd == LC_SEGMENT_64 {
            let segment = unsafe { &*(cmd_ptr as *const SegmentCommand64) };
            let mut section_ptr = unsafe { (segment as *const SegmentCommand64).add(1) as *const Section64 };
            
            for _ in 0..segment.nsects {
                let section = unsafe { &*section_ptr };
                let section_type = section.flags & 0xff;
                
                match section_type {
                    S_THREAD_LOCAL_VARIABLES => {
                        // Found thunks section
                        tlv_sections.push(TLVSectionInfo {
                            thunks_addr: section.addr,
                            thunks_size: section.size,
                            initial_content_addr: 0,  // will be filled later
                            initial_content_size: 0,  // will be filled later
                            all_zero_fill: true,      // will be updated
                        });
                    },
                    S_THREAD_LOCAL_REGULAR => {
                        // Non-zero initial content
                        all_zero_fill = false;
                        if initial_content_start.is_none() {
                            initial_content_start = Some(section.addr);
                        }
                        let end = section.addr + section.size;
                        initial_content_size = end - initial_content_start.unwrap();
                    },
                    S_THREAD_LOCAL_ZEROFILL => {
                        // Zero-fill content
                        if initial_content_start.is_none() {
                            initial_content_start = Some(section.addr);
                        }
                        let end = section.addr + section.size;
                        initial_content_size = end - initial_content_start.unwrap();
                    },
                    _ => {}
                }
                
                section_ptr = unsafe { section_ptr.add(1) };
            }
        }
        
        cmd_ptr = unsafe { cmd_ptr.add(cmd.cmdsize as usize) };
    }
    
    // Update TLV sections with initial content info
    for section in &mut tlv_sections {
        section.initial_content_addr = initial_content_start.unwrap_or(0);
        section.initial_content_size = initial_content_size;
        section.all_zero_fill = all_zero_fill;
        
        // Found TLV section
    }
    
    Ok(tlv_sections)
}
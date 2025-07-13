use windows::Win32::System::Shutdown::*;

fn initiate_shutdown(flags: SHUTDOWN_FLAGS) -> u32 {
    unsafe {
        InitiateShutdownW(
            None, // Computer name (None for local)
            None, // Message to display
            0,    // Timeout in seconds (0 for immediate)
            SHUTDOWN_FORCE_OTHERS | flags,
            SHTDN_REASON_MAJOR_OTHER | SHTDN_REASON_MINOR_OTHER,
        )
    }
}

pub fn shutdown() -> u32{
    initiate_shutdown(SHUTDOWN_POWEROFF)
}

pub fn reboot() -> u32 {
    initiate_shutdown(SHUTDOWN_RESTART)
}
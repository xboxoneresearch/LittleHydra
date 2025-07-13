use crate::error::Error;
use windows::core::s;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::LibraryLoader::{LoadLibraryA, GetProcAddress};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::Security::{DuplicateToken, SECURITY_IMPERSONATION_LEVEL};
use windows::Win32::Security::TOKEN_ALL_ACCESS;

pub type QueryUserTokenFn = unsafe extern "system" fn(
    dwSessionId: u32,
    handle: &mut HANDLE,
) -> bool;

pub fn fetch_query_user_token() -> Result<QueryUserTokenFn, Error> {
    unsafe {
        let hmod = LoadLibraryA(s!("EXT-MS-WIN-SESSION-USERTOKEN-L1-1-0.DLL"))?;
        let func = GetProcAddress(hmod, s!("QueryUserToken"))
            .ok_or(Error::Impersonation("GetProcAddress failed".into()))?;
        let ptr: QueryUserTokenFn = std::mem::transmute(func);
        Ok(ptr)
    }
}

fn duplicate_token(token_handle: HANDLE, impersonation_level: i32) -> Result<HANDLE, Error> {
    let mut duplicated_token = HANDLE::default();
    let result = unsafe {
        // Duplicate the token with the specified impersonation level
        DuplicateToken(
            token_handle,
            SECURITY_IMPERSONATION_LEVEL(impersonation_level),
            &mut duplicated_token,
        )
    };

    if let Err(_) = result {
        return Err(Error::Impersonation("Failed to duplicate token".into()));
    }
    
    Ok(duplicated_token)
}

pub(crate) fn get_defaultaccount_token(impersonation_level: i32) -> Result<HANDLE, Error> {
    #[allow(non_snake_case)]
    let QueryUserToken = fetch_query_user_token()?;

    let mut handle = HANDLE::default();

    unsafe {
        if !QueryUserToken(0, &mut handle) {
            return Err(Error::Impersonation("Failed to query token".into()));
        }
    }

    duplicate_token(handle, impersonation_level)
}

pub fn get_primary_token(impersonation_level: i32) -> Result<HANDLE, Error> {
    unsafe {
        // Get the current process handle
        let process_handle = GetCurrentProcess();
        
        // Open the process token
        let mut token_handle = HANDLE::default();
        let result = OpenProcessToken(
            process_handle,
            TOKEN_ALL_ACCESS,
            &mut token_handle,
        );
        
        if let Err(_) = result {
            return Err(Error::Impersonation("Failed to open process token".into()));
        }
        
        duplicate_token(token_handle, impersonation_level)
    }
}
//! Query-only helpers adapted from X; see third-party/LeagueReplayStudio-NOTICE.txt.
use std::{path::PathBuf, ptr};
use windows_sys::Win32::{
    Foundation::{GetLastError, ERROR_SERVICE_DOES_NOT_EXIST},
    Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives},
    System::{
        Services::{
            CloseServiceHandle, OpenSCManagerW, OpenServiceW, QueryServiceStatus, SC_HANDLE,
            SC_MANAGER_CONNECT, SERVICE_QUERY_STATUS, SERVICE_STATUS, SERVICE_STOPPED,
        },
        WindowsProgramming::DRIVE_FIXED,
    },
};

pub fn fixed_drives() -> Vec<PathBuf> {
    // SAFETY: no pointers or buffers are passed to GetLogicalDrives.
    let drives = unsafe { GetLogicalDrives() };
    (0_u8..26)
        .filter_map(|index| {
            if drives & (1_u32 << index) == 0 {
                return None;
            }
            let root = format!("{}:\\", char::from(b'A' + index));
            let wide: Vec<_> = root.encode_utf16().chain([0]).collect();
            // SAFETY: wide is a live NUL-terminated drive-root string.
            (unsafe { GetDriveTypeW(wide.as_ptr()) } == DRIVE_FIXED).then(|| PathBuf::from(root))
        })
        .collect()
}

pub fn vanguard_active() -> Result<bool, String> {
    struct ServiceHandle(SC_HANDLE);
    impl Drop for ServiceHandle {
        fn drop(&mut self) {
            // SAFETY: this owner holds a valid service handle, closed exactly once.
            unsafe {
                CloseServiceHandle(self.0);
            }
        }
    }
    // SAFETY: null names select the local service manager; only connection access is requested.
    let manager = unsafe { OpenSCManagerW(ptr::null(), ptr::null(), SC_MANAGER_CONNECT) };
    if manager.is_null() {
        return Err("replay.protectionUnavailable".into());
    }
    let manager = ServiceHandle(manager);
    let name: Vec<_> = "vgk".encode_utf16().chain([0]).collect();
    // SAFETY: manager is live and name is NUL-terminated; no service-control permission is requested.
    let service = unsafe { OpenServiceW(manager.0, name.as_ptr(), SERVICE_QUERY_STATUS) };
    if service.is_null() {
        // SAFETY: retrieve the error immediately after OpenServiceW failed.
        return if unsafe { GetLastError() } == ERROR_SERVICE_DOES_NOT_EXIST {
            Ok(false)
        } else {
            Err("replay.protectionUnavailable".into())
        };
    }
    let service = ServiceHandle(service);
    let mut status = SERVICE_STATUS::default();
    // SAFETY: the service handle and writable output structure remain valid for this call.
    if unsafe { QueryServiceStatus(service.0, &mut status) } == 0 {
        return Err("replay.protectionUnavailable".into());
    }
    Ok(status.dwCurrentState != SERVICE_STOPPED)
}

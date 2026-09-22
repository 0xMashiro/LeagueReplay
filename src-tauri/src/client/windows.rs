//! Query-only discovery for Windows clients whose command line sysinfo cannot read.
//! Never log command lines: they contain the local LCU credential.
use super::lcu::LcuError;
use std::{
    ffi::{c_void, OsString},
    mem::{align_of, size_of},
    ops::Range,
    os::windows::{
        ffi::OsStringExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    ptr, slice,
};
use windows_sys::Win32::{
    Foundation::{GetLastError, LocalFree, ERROR_ACCESS_DENIED, HANDLE, UNICODE_STRING},
    System::{
        LibraryLoader::{GetModuleHandleW, GetProcAddress},
        Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    },
    UI::Shell::CommandLineToArgvW,
};
use zeroize::{Zeroize, Zeroizing};

type QueryProcess = unsafe extern "system" fn(HANDLE, i32, *mut c_void, u32, *mut u32) -> i32;
const COMMAND_LINE_INFORMATION: i32 = 60;
const MAX_BYTES: usize = 128 * 1024;

pub fn command_line(pid: u32) -> Result<Vec<OsString>, LcuError> {
    // SAFETY: query-only, non-inheritable handle; ownership moves to OwnedHandle.
    let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if raw.is_null() {
        return Err(if unsafe { GetLastError() } == ERROR_ACCESS_DENIED {
            LcuError::AccessDenied
        } else {
            LcuError::CommandLineUnavailable
        });
    }
    let process = unsafe { OwnedHandle::from_raw_handle(raw) };
    // Resolve the optional NT export dynamically, as recommended by Microsoft.
    // SAFETY: static NUL-terminated names; ntdll stays loaded for the process lifetime.
    let query: QueryProcess = unsafe {
        let module = GetModuleHandleW(windows_sys::core::w!("ntdll.dll"));
        if module.is_null() {
            return Err(LcuError::CommandLineUnavailable);
        }
        let address = GetProcAddress(module, c"NtQueryInformationProcess".as_ptr().cast())
            .ok_or(LcuError::CommandLineUnavailable)?;
        std::mem::transmute(address)
    };
    let mut needed = 0;
    // SAFETY: size probe writes only to `needed` using a live handle.
    let status = unsafe {
        query(
            process.as_raw_handle(),
            COMMAND_LINE_INFORMATION,
            ptr::null_mut(),
            0,
            &mut needed,
        )
    };
    if status == 0xC0000022_u32 as i32 {
        return Err(LcuError::AccessDenied);
    }
    if !(size_of::<UNICODE_STRING>()..=MAX_BYTES).contains(&(needed as usize)) {
        return Err(LcuError::CommandLineUnavailable);
    }
    // usize alignment accommodates UNICODE_STRING; zeroize also covers error paths.
    let mut buffer = Zeroizing::new(vec![0usize; (needed as usize).div_ceil(size_of::<usize>())]);
    let mut returned = 0;
    // SAFETY: the aligned allocation contains at least `needed` writable bytes.
    let status = unsafe {
        query(
            process.as_raw_handle(),
            COMMAND_LINE_INFORMATION,
            buffer.as_mut_ptr().cast(),
            needed,
            &mut returned,
        )
    };
    if status < 0 {
        return Err(if status == 0xC0000022_u32 as i32 {
            LcuError::AccessDenied
        } else {
            LcuError::CommandLineUnavailable
        });
    }
    if returned < size_of::<UNICODE_STRING>() as u32 || returned > needed {
        return Err(LcuError::CommandLineUnavailable);
    }
    // SAFETY: allocation alignment and returned header length were checked above.
    let header = unsafe { &*buffer.as_ptr().cast::<UNICODE_STRING>() };
    let range = string_range(
        buffer.as_ptr() as usize,
        returned as usize,
        header.Buffer as usize,
        header.Length as usize,
        header.MaximumLength as usize,
    )?;
    // SAFETY: range is checked inside our returned allocation and UTF-16 aligned.
    let units = unsafe {
        slice::from_raw_parts(
            buffer.as_ptr().cast::<u8>().add(range.start).cast::<u16>(),
            range.len() / 2,
        )
    };
    parse_arguments(units)
}

fn string_range(
    base: usize,
    bytes: usize,
    pointer: usize,
    length: usize,
    maximum: usize,
) -> Result<Range<usize>, LcuError> {
    let start = pointer
        .checked_sub(base)
        .ok_or(LcuError::CommandLineUnavailable)?;
    let end = start
        .checked_add(length)
        .ok_or(LcuError::CommandLineUnavailable)?;
    if length == 0
        || !length.is_multiple_of(2)
        || maximum < length
        || !pointer.is_multiple_of(align_of::<u16>())
        || start < size_of::<UNICODE_STRING>()
        || end > bytes
    {
        return Err(LcuError::CommandLineUnavailable);
    }
    Ok(start..end)
}

fn parse_arguments(units: &[u16]) -> Result<Vec<OsString>, LcuError> {
    if units.is_empty() || units.contains(&0) {
        return Err(LcuError::CommandLineUnavailable);
    }
    let mut input = Zeroizing::new(units.to_vec());
    input.push(0);
    let mut count = 0;
    // SAFETY: input is a live NUL-terminated UTF-16 string and count is writable.
    let raw = unsafe { CommandLineToArgvW(input.as_ptr(), &mut count) };
    if raw.is_null() {
        return Err(LcuError::CommandLineUnavailable);
    }
    struct Arguments(*mut *mut u16);
    impl Drop for Arguments {
        fn drop(&mut self) {
            // SAFETY: the allocation returned by CommandLineToArgvW is freed once.
            unsafe {
                LocalFree(self.0.cast());
            }
        }
    }
    let owned = Arguments(raw);
    if count <= 0 || count as usize > units.len() + 1 {
        return Err(LcuError::CommandLineUnavailable);
    }
    let mut result = Vec::with_capacity(count as usize);
    for index in 0..count as usize {
        // SAFETY: the OS parser returns count pointers to valid NUL-terminated strings.
        let argument = unsafe { *owned.0.add(index) };
        let mut length = 0;
        while unsafe { *argument.add(length) } != 0 {
            length += 1;
        }
        let text = unsafe { slice::from_raw_parts_mut(argument, length) };
        result.push(OsString::from_wide(text));
        text.zeroize();
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_current_process_and_preserve_quoted_arguments() {
        let args = command_line(std::process::id()).unwrap();
        assert!(!args.is_empty());
        let line: Vec<_> = r#""C:\游戏 路径\LeagueClientUx.exe" "--remoting-auth-token=local-test" --app-port=5000"#.encode_utf16().collect();
        assert_eq!(
            parse_arguments(&line).unwrap(),
            vec![
                OsString::from(r"C:\游戏 路径\LeagueClientUx.exe"),
                "--remoting-auth-token=local-test".into(),
                "--app-port=5000".into()
            ]
        );
    }

    #[test]
    fn rejects_out_of_bounds_or_misaligned_native_strings() {
        assert_eq!(string_range(0x1000, 64, 0x1010, 10, 12).unwrap(), 16..26);
        for (pointer, length, maximum) in [
            (0xFFF, 10, 10),
            (0x1000, 10, 10),
            (0x1011, 10, 10),
            (0x1010, 9, 10),
            (0x1010, 50, 50),
            (0x1010, 10, 8),
            (usize::MAX - 1, 10, 10),
        ] {
            assert!(string_range(0x1000, 64, pointer, length, maximum).is_err());
        }
    }
}

//! Starting the daemon on Windows so that it holds nothing of its parent's.
//!
//! CreateProcess hands a child every inheritable handle of its parent, not only the three in its startup info.
//! A daemon started by a CLI that an agent harness or `ssh` runs with pipes would keep those pipes open, and the
//! caller would wait for their end until the daemon exits fifteen idle minutes later. `std::process::Command`
//! cannot restrict this on stable Rust, so the daemon is created with an explicit handle list.

#![allow(unsafe_code)]

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, OwnedHandle};
use std::path::Path;

use windows_sys::Win32::Foundation::{
    CloseHandle, HANDLE, HANDLE_FLAG_INHERIT, SetHandleInformation,
};
use windows_sys::Win32::System::Threading::{
    CREATE_NEW_PROCESS_GROUP, CREATE_UNICODE_ENVIRONMENT, CreateProcessW, DETACHED_PROCESS,
    DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT, InitializeProcThreadAttributeList,
    PROC_THREAD_ATTRIBUTE_HANDLE_LIST, PROCESS_INFORMATION, STARTF_USESTDHANDLES, STARTUPINFOEXW,
    UpdateProcThreadAttribute,
};

fn inheritable(handle: &impl AsRawHandle) -> io::Result<HANDLE> {
    let raw = handle.as_raw_handle() as HANDLE;
    if unsafe { SetHandleInformation(raw, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(raw)
}

/// An environment block for `CreateProcessW`: `name=value` strings in UTF-16, sorted by name without regard to case
/// as the documentation of `CreateProcessW` requires, ending with an empty string.
fn block(env: &[(OsString, OsString)]) -> Vec<u16> {
    let mut sorted: Vec<&(OsString, OsString)> = env.iter().collect();
    sorted.sort_by_key(|(name, _)| name.to_string_lossy().to_uppercase());
    let mut block = Vec::new();
    for (name, value) in sorted {
        block.extend(name.encode_wide());
        block.push(u16::from(b'='));
        block.extend(value.encode_wide());
        block.push(0);
    }
    if block.is_empty() {
        block.push(0);
    }
    block.push(0);
    block
}

/// Runs `exe daemon --detach` with `NUL` as stdin, `log` as stderr, a new pipe as stdout and `env` as its whole
/// environment, and returns the pipe's read end. No other handle of this process reaches the daemon.
pub fn spawn_daemon(exe: &Path, log: &File, env: &[(OsString, OsString)]) -> io::Result<File> {
    let nul = std::fs::OpenOptions::new().read(true).open("NUL")?;
    let (reader, writer) = std::io::pipe()?;
    let handles = [inheritable(&nul)?, inheritable(&writer)?, inheritable(log)?];

    let mut size = 0usize;
    unsafe { InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut size) };
    let mut list = vec![0u8; size];
    let list_ptr = list.as_mut_ptr().cast();
    if unsafe { InitializeProcThreadAttributeList(list_ptr, 1, 0, &mut size) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let updated = unsafe {
        UpdateProcThreadAttribute(
            list_ptr,
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
            handles.as_ptr().cast(),
            std::mem::size_of_val(&handles),
            std::ptr::null_mut(),
            std::ptr::null(),
        )
    };
    if updated == 0 {
        let e = io::Error::last_os_error();
        unsafe { DeleteProcThreadAttributeList(list_ptr) };
        return Err(e);
    }

    let mut startup: STARTUPINFOEXW = unsafe { std::mem::zeroed() };
    startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = handles[0];
    startup.StartupInfo.hStdOutput = handles[1];
    startup.StartupInfo.hStdError = handles[2];
    startup.lpAttributeList = list_ptr;

    let wide = |s: &OsStr| s.encode_wide().chain([0]).collect::<Vec<u16>>();
    let program = wide(exe.as_os_str());
    let mut command = wide(OsStr::new(&format!(
        "\"{}\" daemon --detach",
        exe.display()
    )));
    let environment = block(env);
    let mut process: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
    let created = unsafe {
        CreateProcessW(
            program.as_ptr(),
            command.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
            EXTENDED_STARTUPINFO_PRESENT
                | DETACHED_PROCESS
                | CREATE_NEW_PROCESS_GROUP
                | CREATE_UNICODE_ENVIRONMENT,
            environment.as_ptr().cast(),
            std::ptr::null(),
            &startup.StartupInfo,
            &mut process,
        )
    };
    let failure = (created == 0).then(io::Error::last_os_error);
    unsafe {
        DeleteProcThreadAttributeList(list_ptr);
        if failure.is_none() {
            CloseHandle(process.hThread);
            CloseHandle(process.hProcess);
        }
    }
    if let Some(e) = failure {
        return Err(e);
    }
    drop(writer);
    Ok(File::from(OwnedHandle::from(reader)))
}

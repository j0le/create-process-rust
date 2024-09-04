use std::{
    convert::AsRef,
    ffi::OsStr,
    os::windows::ffi::OsStrExt,
};

use windows::Win32::System::Threading as WinThreading;
use windows::core::{
    PCWSTR,
    PWSTR,
};
use windows::Win32::Foundation::{
    HANDLE,
    CloseHandle,
    WAIT_OBJECT_0,
    WIN32_ERROR,
};


pub fn create_process
    <S1: AsRef<OsStr>, S2: AsRef<OsStr>>
    (program_opt: Option<S1>, cmd_opt: Option<S2>)
    -> Result<u32,String>
{
    let startup_info : WinThreading::STARTUPINFOW = WinThreading::STARTUPINFOW{
        cb: u32::try_from(std::mem::size_of::<WinThreading::STARTUPINFOW>()).unwrap(),
        lpReserved: PWSTR::null(),
        lpDesktop: PWSTR::null(),
        lpTitle: PWSTR::null(),
        dwX: 0,
        dwY: 0,
        dwXSize: 0,
        dwYSize: 0,
        dwXCountChars: 0,
        dwYCountChars: 0,
        dwFillAttribute: 0,
        dwFlags: WinThreading::STARTUPINFOW_FLAGS(0),
        wShowWindow: 0,
        cbReserved2: 0,
        lpReserved2: std::ptr::null_mut(),
        hStdInput: HANDLE::default(),
        hStdOutput: HANDLE::default(),
        hStdError: HANDLE::default(),
    };
    let creation_flags = WinThreading::PROCESS_CREATION_FLAGS(0);
    let mut process_information = WinThreading::PROCESS_INFORMATION::default();

    let mut program_vec_u16 : Vec<u16>;
    let program_pcwstr: PCWSTR = match program_opt{
        None => PCWSTR::null(),
        Some(os_str) => {
            program_vec_u16 = OsStrExt::encode_wide(os_str.as_ref()).collect();
            program_vec_u16.push(0u16); // Push null terminator
            PCWSTR::from_raw(program_vec_u16.as_ptr())
        },
    };

    let mut cmd_vec_u16 : Vec<u16>;
    let cmd_pwstr: PWSTR = match cmd_opt{
        None => PWSTR::null(),
        Some(os_str) => {
            cmd_vec_u16 = OsStrExt::encode_wide(os_str.as_ref()).collect();
            cmd_vec_u16.push(0u16); // Push null terminator
            PWSTR::from_raw(cmd_vec_u16.as_mut_ptr())
        },
    };

    if ! unsafe{ WinThreading::CreateProcessW(
            program_pcwstr,
            cmd_pwstr,
            None,
            None,
            false,
            creation_flags,
            None,
            PCWSTR::null(),
            &startup_info,
            &mut process_information
        )}.as_bool()
    {
        return Err("CreateProcessW failed!".to_string());
    };

    if ! process_information.hThread.is_invalid() {
        if !unsafe {CloseHandle(process_information.hThread)}.as_bool() {
            eprintln!("Warning: Closing thread handle failed.");
        }
        process_information.hThread = HANDLE::default();
    }
    else {
        eprintln!("Warning: Thread handle is invalid.");
    }

    if process_information.hProcess.is_invalid() {
        return Err("Process handle is invalid.".to_string())
    }

    let wait_result: WIN32_ERROR = unsafe {
        WinThreading::WaitForSingleObject(process_information.hProcess, WinThreading::INFINITE)
    };

    let mut result : Result<u32, String> =
        if wait_result == WAIT_OBJECT_0 {
            let mut status : u32 = 0;
            if ! unsafe {WinThreading::GetExitCodeProcess(process_information.hProcess, &mut status)}.as_bool() {
                Err("Failed to get exit code of process".to_string())
            }else{
                Ok(status)
            }
        }else{
            Err("Failed to wait for process to exit.".to_string())
        };

    if !unsafe {CloseHandle(process_information.hProcess)}.as_bool() {
        match result{
            Ok(..) => result = Err("Failed to close process handle.".to_string()),
            _ => {}
        }
    }
    process_information.hProcess = HANDLE::default();

    return result;
}

use std::ffi::{OsStr,OsString};
use std::os::windows::ffi::{OsStrExt,OsStringExt};
use std::path::PathBuf;
use std::process::Command;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::UI::Shell::GetUserProfileDirectoryW;
use windows::core::PWSTR;

mod murks;

// - The goal is, to create a commandline with invalid UTF16
// - Another goal is to crate a commandline with a character that, if encoded as UTF16, has one byte
//   that is a ASCII Space character

fn home_dir() -> Option<PathBuf> {
    // https://github.com/rust-lang/rust/pull/90144
    // "Use a hardcoded constant instead of calling OpenProcessToken"
    let current_process_token : HANDLE = HANDLE{ 0 : -4isize};
    const NUMBER_OF_WCHARS: u32 = 512u32;
    // FIXME: Zero initialization is unnecessary. Consider using std::mem::MaybeUninit.
    let mut u16_array : [u16; NUMBER_OF_WCHARS as usize] = [0u16; NUMBER_OF_WCHARS as usize];
    let buf = PWSTR::from_raw(u16_array.as_mut_ptr());
    let mut acutal_size_in_wchars: u32 = NUMBER_OF_WCHARS;
    unsafe {
        if GetUserProfileDirectoryW(current_process_token, buf, &mut acutal_size_in_wchars as *mut u32).as_bool(){
            if acutal_size_in_wchars > NUMBER_OF_WCHARS || acutal_size_in_wchars == 0u32 {
                panic!("Unexpected return value of GetUserProfileDirectoryW");
            }else{
                let size_without_terminating_nul = acutal_size_in_wchars-1;
                return Some(PathBuf::from(OsString::from_wide(&u16_array[0..size_without_terminating_nul as usize])));
            }
        }
        else {
            return None;
        }
    };
}

fn print_os_str<O: AsRef<OsStr>>(os_str: O) {
    let os_str : &OsStr = os_str.as_ref();
    print!("as_encoded_bytes");
    for b in os_str.as_encoded_bytes() {
        print!(" {:02x}", b);
    }
    println!("");
    print!("encode_wide:");
    for bb in os_str.encode_wide() {
        print!(" {:04x}", bb);
    }
    println!("");
}

fn main() {
    murks::hello();
    let smiley = OsStr::new("\u{1f600}");
    print_os_str(&smiley);
    let smiley_text = {
        let mut smiley_text = OsString::new();
        smiley_text.push("Text mit Smiley ");
        smiley_text.push(smiley);
        smiley_text
    };


    let high_surrogate = OsString::from_wide(&[0xd83du16]);
    let low_surrogate  = OsString::from_wide(&[0xde00u16]);
    print_os_str(&high_surrogate);
    let combined = {
        let mut combined = OsString::new();
        combined.push("(");
        combined.push(&high_surrogate);
        combined.push("-");
        combined.push(&low_surrogate);
        combined.push(")");
        combined
    };

    let home_dir = home_dir().expect("couldn't get home directory");

    println!("Home dir: {}", home_dir.to_string_lossy());

    let append_to_homedir = |extra| {
        let mut p = home_dir.clone();
        p.push(extra);
        p
    };
    let get_command_line: PathBuf = append_to_homedir("prog\\get-command-line\\x64\\Debug\\get-command-line.exe");
    let pargs:            PathBuf = append_to_homedir("prog\\create-process-rust\\cpp\\build.d\\pargs.exe");
    let pargs_utf8:       PathBuf = append_to_homedir("prog\\create-process-rust\\cpp\\build.d\\pargs-utf8.exe");
    let cpr:              PathBuf = append_to_homedir("prog\\create-process-rust\\cpr.exe");

    let commands = [
        Command::new(&get_command_line),
        Command::new(&pargs),
        Command::new(&pargs_utf8),
        {let mut c = Command::new(&cpr); c.arg("--print-args-only"); c},
        {let mut c = Command::new(&cpr); c.arg("--json").arg("--print-args-only"); c},
    ];

    for mut command in commands {
        println!("starting {}", command.get_program().to_string_lossy());
        let mut child = command
            .arg(smiley)
            .arg("Moin Tach")
            .arg(&smiley_text)
            .arg("another one")
            .arg(&combined)
            .spawn()
            .expect("pargs failed to start");

        child.wait().expect("wait failed");
        print!("\n\n");
    }

}

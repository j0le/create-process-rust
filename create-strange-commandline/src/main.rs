use std::process::Command;
use std::ffi::{OsStr,OsString};
use  std::os::windows::ffi::{OsStrExt,OsStringExt};

// - The goal is, to create a commandline with invalid UTF16
// - Another goal is to crate a commandline with a character that, if encoded as UTF16, has one byte
//   that is a ASCII Space character

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
    println!("Hello, world!");
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


    const GET_COMMAND_LINE: &str = "C:\\Users\\USER\\prog\\get-command-line\\x64\\Debug\\get-command-line.exe";
    const PARGS: &str = "C:\\Users\\USER\\prog\\create-process-rust\\cpp\\pargs.exe";
    const PARGS_UTF8: &str = "C:\\Users\\USER\\prog\\create-process-rust\\cpp\\pargs-utf8.exe";
    const CPR: &str = "C:\\Users\\USER\\prog\\create-process-rust\\cpr.exe";
    let commands = [
        Command::new(GET_COMMAND_LINE),
        Command::new(PARGS),
        Command::new(PARGS_UTF8),
        {let mut c = Command::new(CPR); c.arg("--print-args-only"); c},
        {let mut c = Command::new(CPR); c.arg("--json").arg("--print-args-only"); c},
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

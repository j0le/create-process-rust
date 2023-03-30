
use std::{
    env, 
    ffi::OsString,
    io,
    io::Write,
    os::windows::ffi::OsStringExt,
    thread, 
    time, 
};

use windows::{
    core::PWSTR,
    Win32::System::Environment,
};



fn main() {

    let cmdline:OsString = unsafe {
        let cmdline_ptr:PWSTR = Environment::GetCommandLineW();
        if cmdline_ptr.is_null() {
            println!("couldn't get commandline");
            return;
        }
        OsStringExt::from_wide(cmdline_ptr.as_wide())
    };

    let cmdline = match cmdline.to_str() {
        Some(str) => {
            println!("Die Kommandozeile konnte verlustfrei konvertiert werden.");
            std::borrow::Cow::from(str)
        },
        None => {
            println!("Die Kommandozeile muste verlustbehaftet konvertiert werden.");
            cmdline.to_string_lossy()
        }
    };
    println!("Die command line sieht wie folgt aus,\
              aber ohne die spitzen Anführungszeichen (»«): \n\
              »{}«\n", cmdline);



    let args: Vec<String> = env::args().collect();
    for n in 0..args.len() {
        println!("Das {}. Argument ist: »{}«", n, &args[n]);
    }
    
    loop {
        print!(".");
        match io::stdout().flush() {
            Err(..) => {println!("Cannot flush!"); return;},
            Ok(()) => {},
        };
        thread::sleep(time::Duration::from_millis(2000));
    }
}

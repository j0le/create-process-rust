
use std::{
    env, 
    ffi::OsString,
    io,
    io::Write,
    os::windows::ffi::OsStringExt,
    thread, 
    time, 
};

use windows::Win32::System::Environment ;



fn main() {

    let mut cmdline_vec : Vec<u16> = vec!();
    unsafe {
        let cmdline_ptr = Environment::GetCommandLineW();
        if cmdline_ptr.is_null() {
            println!("couldn't get commandline");
            return;
        }
        let mut cmdline_ptr = cmdline_ptr.as_ptr();
        loop {
            let c:u16 = *cmdline_ptr;
            if c == 0 { break; }
            cmdline_vec.push(c);
            cmdline_ptr = cmdline_ptr.add(1);
        }
    }

    let cmdline : OsString = OsStringExt::from_wide(cmdline_vec.as_slice());
    match cmdline.to_str() {
        Some(str) => {
            println!("could convert commandline losslesly.");
            println!("Die command line sieht wie folgt aus:\n»{}«", str);
        },
        None => {
            let str = cmdline.to_string_lossy();
            println!("Die command line sieht wie folgt aus: »{}«", str);
        }
    };

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

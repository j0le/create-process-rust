
mod wstr;

//use crate::wstr::WStrUnits;

use std::{
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


struct Arg<'lifetime_of_slice> {
    arg: OsString,
    range: std::ops::Range<usize>,
    raw: &'lifetime_of_slice[u16],
}


struct ArgListBuilder<'a> {
    cmd_line: &'a [u16],
    cur: Vec<u16>,
    start_index: usize,
    end_index: usize,
    arg_list: Vec<Arg<'a>>,
}

impl<'a> ArgListBuilder<'a> {

    fn new(cmd_line: &'a [u16]) -> Self {
        Self {
            cmd_line,
            cur: vec![],
            start_index: 0,
            end_index: 0,
            arg_list: vec![]
        }
    }

    fn push_arg(&mut self){
        let range = self.start_index..(self.end_index-1); // TODO try ..=
        self.arg_list.push(Arg{
            arg: OsString::from_wide(&self.cur),
            range:range.clone(),
            raw: &self.cmd_line[range],
        });
        self.cur.truncate(0);
    }
    fn peek(&self) -> Option<u16> {
        self.cmd_line.get(self.end_index).map(|w:&u16| *w)
    }
    fn next(&mut self) -> Option<u16> {
        let opt_w = self.peek();
        if self.end_index <= self.cmd_line.len() {
            self.end_index += 1;
        }
        opt_w
    }

    fn get_current(&mut self) -> &mut Vec<u16>{
        &mut self.cur
    }

    fn set_start_index(&mut self){
        self.start_index = self.end_index;
    }

    fn advance_while<P : FnMut(u16) -> bool>(&mut self, mut predicate: P) -> usize {
        let mut counter = 0;
        while self.end_index < self.cmd_line.len() {
            if !predicate(self.cmd_line[self.end_index]) {
                break;
            }
            self.end_index += 1;
            counter += 1;
        }
        counter
    }

    fn get_arg_list(self) -> Vec<Arg<'a>> {
        self.arg_list
    }
}


// from ~/.rustup/toolchains/stable-x86_64-pc-windows-msvc/lib/rustlib/src/rust/library/std/src/sys/windows/args.rs
/// Implements the Windows command-line argument parsing algorithm.
///
/// Microsoft's documentation for the Windows CLI argument format can be found at
/// <https://docs.microsoft.com/en-us/cpp/cpp/main-function-command-line-args?view=msvc-160#parsing-c-command-line-arguments>
///
/// A more in-depth explanation is here:
/// <https://daviddeley.com/autohotkey/parameters/parameters.htm#WIN>
///
/// Windows includes a function to do command line parsing in shell32.dll.
/// However, this is not used for two reasons:
///
/// 1. Linking with that DLL causes the process to be registered as a GUI application.
/// GUI applications add a bunch of overhead, even if no windows are drawn. See
/// <https://randomascii.wordpress.com/2018/12/03/a-not-called-function-can-cause-a-5x-slowdown/>.
///
/// 2. It does not follow the modern C/C++ argv rules outlined in the first two links above.
///
/// This function was tested for equivalence to the C/C++ parsing rules using an
/// extensive test suite available at
/// <https://github.com/ChrisDenton/winarg/tree/std>.
fn parse_lp_cmd_line<'a>(cmd_line: &'a [u16], handle_first_special: bool) -> Vec<Arg<'a>> {
    const BACKSLASH: u16 = b'\\' as u16;
    const QUOTE: u16 = b'"' as u16;
    const TAB: u16 = b'\t' as u16;
    const SPACE: u16 = b' ' as u16;

    // If the cmd line pointer is null or it points to an empty string then
    // return an empty vector.
    if cmd_line.is_empty() {
        return Vec::<Arg<'a>>::new();
    }

    let mut builder = ArgListBuilder::new(cmd_line);
    let mut in_quotes = false;

    // The executable name at the beginning is special.
    if handle_first_special {
        while let Some(w) = builder.next() {
            match w {
                // A quote mark always toggles `in_quotes` no matter what because
                // there are no escape characters when parsing the executable name.
                QUOTE => in_quotes = !in_quotes,
                // If not `in_quotes` then whitespace ends argv[0].
                SPACE | TAB if !in_quotes => break,
                // In all other cases the code unit is taken literally.
                _ => builder.get_current().push(w),
            }
        }
        builder.push_arg();
        // Skip whitespace.
        builder.advance_while(|w| w == SPACE || w == TAB);
        builder.set_start_index();
    }

    // Parse the arguments according to these rules:
    // * All code units are taken literally except space, tab, quote and backslash.
    // * When not `in_quotes`, space and tab separate arguments. Consecutive spaces and tabs are
    // treated as a single separator.
    // * A space or tab `in_quotes` is taken literally.
    // * A quote toggles `in_quotes` mode unless it's escaped. An escaped quote is taken literally.
    // * A quote can be escaped if preceded by an odd number of backslashes.
    // * If any number of backslashes is immediately followed by a quote then the number of
    // backslashes is halved (rounding down).
    // * Backslashes not followed by a quote are all taken literally.
    // * If `in_quotes` then a quote can also be escaped using another quote
    // (i.e. two consecutive quotes become one literal quote).
    in_quotes = false;
    while let Some(w) = builder.next() {
        match w {
            // If not `in_quotes`, a space or tab ends the argument.
            SPACE | TAB if !in_quotes => {
                builder.push_arg();

                // Skip whitespace.
                builder.advance_while(|w| w == SPACE || w == TAB);
                builder.set_start_index();
            }
            // Backslashes can escape quotes or backslashes but only if consecutive backslashes are followed by a quote.
            BACKSLASH => {
                let backslash_count = builder.advance_while(|w| w == BACKSLASH) + 1;
                if builder.peek() == Some(QUOTE) {
                    builder.get_current().extend(std::iter::repeat(BACKSLASH).take(backslash_count / 2));
                    // The quote is escaped if there are an odd number of backslashes.
                    if backslash_count % 2 == 1 {
                        builder.next(); // consume the peeked quote
                        builder.get_current().push(QUOTE);
                    }
                } else {
                    // If there is no quote on the end then there is no escaping.
                    builder.get_current().extend(std::iter::repeat(BACKSLASH).take(backslash_count));
                }
            }
            // If `in_quotes` and not backslash escaped (see above) then a quote either
            // unsets `in_quotes` or is escaped by another quote.
            QUOTE if in_quotes => match builder.peek() {
                // Two consecutive quotes when `in_quotes` produces one literal quote.
                Some(QUOTE) => {
                    builder.next(); // consume the peeked quote
                    builder.get_current().push(QUOTE);
                }
                // Otherwise set `in_quotes`.
                Some(_) => in_quotes = false,
                // The end of the command line, so this is the cycle/pass of the loop.
                // After the loop, the current argument gets pushed, because `in_quotes` is true.
                None => {}
            },
            // If not `in_quotes` and not BACKSLASH escaped (see above) then a quote sets `in_quotes`.
            QUOTE => in_quotes = true,
            // Everything else is always taken literally.
            _ => builder.get_current().push(w),
        }
    }
    // Push the final argument, if any.
    if !builder.get_current().is_empty() || in_quotes {
        builder.push_arg();
    }
    builder.get_arg_list()
}

fn main() {
    let cmdline_ptr:PWSTR;
    let cmdline: &[u16] = unsafe {
        cmdline_ptr = Environment::GetCommandLineW();
        if cmdline_ptr.is_null() {
            println!("couldn't get commandline");
            return;
        }
        cmdline_ptr.as_wide()
    };
    let parsed_args_list = parse_lp_cmd_line(cmdline, true);

    let cmdline_os_string : OsString = OsStringExt::from_wide(cmdline);
    let cmdline_u8 = match cmdline_os_string.to_str() {
        Some(str) => {
            println!("Die Kommandozeile konnte verlustfrei konvertiert werden.");
            std::borrow::Cow::from(str)
        },
        None => {
            println!("Die Kommandozeile muste verlustbehaftet konvertiert werden.");
            cmdline_os_string.to_string_lossy()
        }
    };
    println!("Die command line sieht wie folgt aus, \
              aber ohne die spitzen Anführungszeichen (»«): \n\
              »{}«\n", cmdline_u8);

    let mut n : usize = 0;
    for Arg {arg, range, raw} in parsed_args_list {
        let (lossless_or_lossy, arg) = match arg.to_str() {
            Some(arg) => ("lossless:", std::borrow::Cow::from(arg)),
            None      => ("lossy:   ", arg.to_string_lossy()),
        };
        let raw = OsString::from_wide(raw);
        let raw = raw.to_string_lossy();
        println!("Argument {:2}, {:3} .. {:3}, {} »{}«, raw: »{}«",
                 n, range.start, range.end, lossless_or_lossy, arg, raw);
        n += 1;
    }

    if false {
        loop {
            print!(".");
            match io::stdout().flush() {
                Err(..) => {println!("Cannot flush!"); return;},
                Ok(()) => {},
            };
            thread::sleep(time::Duration::from_millis(2000));
        }
    }
}


// What commandline options do I want to have?
// We do these:
// --program <program>
// --program=<program>
// --program-is-null
// --program-from-cmd-line
// --cmd-line-in-arg <commandline>
// --cmd-line-in-arg=<commandline>
// --cmd-line-is-rest <args>...
// --prepend-program

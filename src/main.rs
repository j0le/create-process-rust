
mod wstr;

use crate::wstr::WStrUnits;

use std::{
    ffi::OsString,
    io,
    io::Write,
    os::windows::ffi::OsStringExt,
    os::windows::ffi::OsStrExt,
    thread, 
    time, 
    num::NonZeroU16
};

use windows::{
    core::PWSTR,
    Win32::System::Environment,
};

// from ~/.rustup/toolchains/stable-x86_64-pc-windows-msvc/lib/rustlib/src/rust/library/std/src/sys/windows/args.rs
const fn non_zero_u16(n: u16) -> NonZeroU16 {
    match NonZeroU16::new(n) {
        Some(n) => n,
        None => panic!("called `unwrap` on a `None` value"),
    }
}

struct Arg {
    arg: OsString,
    range: std::ops::Range<usize>,
}

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
fn parse_lp_cmd_line<'a>( lp_cmd_line: Option<WStrUnits<'a>>,) -> Vec<Arg> {
    const BACKSLASH: NonZeroU16 = non_zero_u16(b'\\' as u16);
    const QUOTE: NonZeroU16 = non_zero_u16(b'"' as u16);
    const TAB: NonZeroU16 = non_zero_u16(b'\t' as u16);
    const SPACE: NonZeroU16 = non_zero_u16(b' ' as u16);

    let mut ret_val = Vec::<Arg>::new();
    // If the cmd line pointer is null or it points to an empty string then
    // return an empty vector.
    if lp_cmd_line.as_ref().and_then(|cmd| cmd.peek()).is_none() {
        return ret_val;
    }
    let mut code_units = lp_cmd_line.unwrap();

    // The executable name at the beginning is special.
    let mut in_quotes = false;
    let mut cur = Vec::new();
    let mut index = code_units.get_index();
    for w in &mut code_units {
        match w {
            // A quote mark always toggles `in_quotes` no matter what because
            // there are no escape characters when parsing the executable name.
            QUOTE => in_quotes = !in_quotes,
            // If not `in_quotes` then whitespace ends argv[0].
            SPACE | TAB if !in_quotes => break,
            // In all other cases the code unit is taken literally.
            _ => cur.push(w.get()),
        }
    }
    ret_val.push(Arg{
        arg: OsString::from_wide(&cur),
        range: index..(code_units.get_index().checked_sub(1).unwrap()),
    });
    // Skip whitespace.
    code_units.advance_while(|w| w == SPACE || w == TAB);

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
    let mut cur = Vec::new();
    let mut in_quotes = false;
    index = code_units.get_index();
    while let Some(w) = code_units.next() {
        match w {
            // If not `in_quotes`, a space or tab ends the argument.
            SPACE | TAB if !in_quotes => {
                ret_val.push(Arg{
                    arg: OsString::from_wide(&cur[..]),
                    range: index..(code_units.get_index().checked_sub(1).unwrap()),
                });
                cur.truncate(0);

                // Skip whitespace.
                code_units.advance_while(|w| w == SPACE || w == TAB);
                index = code_units.get_index();
            }
            // Backslashes can escape quotes or backslashes but only if consecutive backslashes are followed by a quote.
            BACKSLASH => {
                let backslash_count = code_units.advance_while(|w| w == BACKSLASH) + 1;
                if code_units.peek() == Some(QUOTE) {
                    cur.extend(std::iter::repeat(BACKSLASH.get()).take(backslash_count / 2));
                    // The quote is escaped if there are an odd number of backslashes.
                    if backslash_count % 2 == 1 {
                        code_units.next();
                        cur.push(QUOTE.get());
                    }
                } else {
                    // If there is no quote on the end then there is no escaping.
                    cur.extend(std::iter::repeat(BACKSLASH.get()).take(backslash_count));
                }
            }
            // If `in_quotes` and not backslash escaped (see above) then a quote either
            // unsets `in_quote` or is escaped by another quote.
            QUOTE if in_quotes => match code_units.peek() {
                // Two consecutive quotes when `in_quotes` produces one literal quote.
                Some(QUOTE) => {
                    cur.push(QUOTE.get());
                    code_units.next();
                }
                // Otherwise set `in_quotes`.
                Some(_) => in_quotes = false,
                // The end of the command line.
                // Push `cur` even if empty, which we do by breaking while `in_quotes` is still set.
                None => break,
            },
            // If not `in_quotes` and not BACKSLASH escaped (see above) then a quote sets `in_quote`.
            QUOTE => in_quotes = true,
            // Everything else is always taken literally.
            _ => cur.push(w.get()),
        }
    }
    // Push the final argument, if any.
    if !cur.is_empty() || in_quotes {
        ret_val.push(Arg{
            arg: OsString::from_wide(&cur[..]),
            range: index..code_units.get_index(),
        });
    }
    ret_val
}

fn main() {

    let (wstr_iter,cmdline): (Option<WStrUnits>, OsString) = unsafe {
        let cmdline_ptr:PWSTR = Environment::GetCommandLineW();
        if cmdline_ptr.is_null() {
            println!("couldn't get commandline");
            return;
        }
        (
            WStrUnits::new(cmdline_ptr.as_ptr()),
            OsStringExt::from_wide(cmdline_ptr.as_wide()),
        )
    };
    let parsed_args_list = parse_lp_cmd_line(wstr_iter);

    let cmdline_u8 = match cmdline.to_str() {
        Some(str) => {
            println!("Die Kommandozeile konnte verlustfrei konvertiert werden.");
            std::borrow::Cow::from(str)
        },
        None => {
            println!("Die Kommandozeile muste verlustbehaftet konvertiert werden.");
            cmdline.to_string_lossy()
        }
    };
    println!("Die command line sieht wie folgt aus, \
              aber ohne die spitzen Anführungszeichen (»«): \n\
              »{}«\n", cmdline_u8);

    let mut n : usize = 0;
    for Arg {arg, range} in parsed_args_list {
        let (lossless_or_lossy, arg) = match arg.to_str() {
            Some(arg) => ("lossless:", std::borrow::Cow::from(arg)),
            None      => ("lossy:   ", arg.to_string_lossy()),
        };
        let x = OsStrExt::encode_wide(cmdline.as_os_str()).collect::<Vec<u16>>();
        let x = &(&x)[range.clone()];
        let x : OsString = OsStringExt::from_wide(&x);
        let x = x.to_string_lossy();
        println!("Argument {:2}, {:3} .. {:3}, {} »{}«, raw: »{}«",
                 n, range.start, range.end, lossless_or_lossy, arg, x);
        n += 1;
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

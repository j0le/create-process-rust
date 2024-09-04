use std::{
    borrow::Cow,
    convert::AsRef,
    error::Error,
    ffi::OsStr,
    ffi::OsString,
    fmt,
    io,
    io::Write,
    os::windows::ffi::OsStrExt,
    os::windows::ffi::OsStringExt,
    thread,
    time,
};
use windows::Win32::System::Environment;

pub(super) struct Arg<'lifetime_of_slice> {
    pub(super) arg: OsString,
    pub(super) range: std::ops::Range<usize>,
    pub(super) raw: &'lifetime_of_slice[u16],
    pub(super) number: usize,
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
            number: self.arg_list.len(),
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

pub fn get_rest<'a>(cmd_line:&'a[u16], arg: &Arg<'a>) -> &'a[u16]{
    let start_of_rest = arg.range.end + 1;
    match cmd_line.get(start_of_rest) {
        Some(_) => {
            &cmd_line[start_of_rest..]
        },
        None => &[],
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
pub(super) fn parse_lp_cmd_line<'a>(cmd_line: &'a [u16], handle_first_special: bool) -> Vec<Arg<'a>> {
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

pub fn get_command_line() -> Result<&'static [u16], &'static str> {
    unsafe {
        let cmdline_ptr : *const u16 = Environment::GetCommandLineW().0;
        if cmdline_ptr.is_null() {
            return Err("Couldn't get commandline");
        }

        let mut len : usize = 0usize;
        let mut moving_ptr : *const u16 = cmdline_ptr;
        while 0u16 != *moving_ptr {
            len = len.checked_add(1usize).ok_or("Interger Overflow")?;
            moving_ptr = moving_ptr.add(1);
        }
        Ok(std::slice::from_raw_parts::<'static, u16>(cmdline_ptr, len))
    }
}


pub struct EscapedArgZero<'a>{
    pub escaped : Cow<'a, [u16]>,
    pub warning: Option<&'static str>,
}

pub fn escape_arg_zero<'a>(slice_utf16: &'a [u16], force_quotes: bool)-> Result<EscapedArgZero<'a>, String> {
    const BACKSLASH: u16 = b'\\' as u16;
    const QUOTE: u16 = b'"' as u16;
    const TAB: u16 = b'\t' as u16;
    const SPACE: u16 = b' ' as u16;

    let mut add_quotes : bool = force_quotes;
    for u in slice_utf16 {
        match *u {
            SPACE | TAB => add_quotes = true,
            QUOTE => return Err("Quotes are not allowed in Argument zero".to_owned()),
            _ => {},
        }

    };
    if slice_utf16.is_empty() {
        add_quotes = true
    }
    Ok(
        if add_quotes {
            EscapedArgZero{
                escaped: {
                    let mut vec_utf16 : Vec<u16> = std::vec::Vec::with_capacity(2usize + slice_utf16.len());
                    vec_utf16.push(QUOTE);
                    vec_utf16.extend_from_slice(slice_utf16);
                    vec_utf16.push(QUOTE);
                    std::borrow::Cow::from(vec_utf16)
                },
                warning: match slice_utf16.last() {
                    Some(&BACKSLASH) => Some("Escaped arg zero ends with backslash and quote: »\\\"«."),
                    _ => None,
                },
            }
        }
        else{
            EscapedArgZero{
                escaped: std::borrow::Cow::from(slice_utf16),
                warning: None,
            }
        }
    )
}

fn ensure_no_nuls<O: AsRef<OsStr>>(os_str: O) -> Result<(), String>{
    let os_str : &OsStr = os_str.as_ref();
    if os_str.encode_wide().any(|u| u == 0u16) {
        return Err("OsStr contains a NUL character".to_owned());
    }
    Ok(())
}

pub(super) fn append_arg<O: AsRef<OsStr>>(cmdline: &mut Vec<u16>, arg: O, force_quotes: bool, raw: bool) -> Result<(), String> {
    const BACKSLASH: u16 = b'\\' as u16;
    const QUOTE: u16 = b'"' as u16;
    const TAB: u16 = b'\t' as u16;
    const SPACE: u16 = b' ' as u16;

    ensure_no_nuls(&arg)?;
    let arg : &OsStr = arg.as_ref();

    let (quote, escape) : (bool, bool) = 
        if raw { (false, false) } 
        else { (force_quotes || arg.is_empty() || arg.encode_wide().any(|c| c == SPACE || c == TAB), true) };

    if quote {
        cmdline.push(QUOTE);
    }

    let mut backslashes: usize = 0;
    for x in arg.encode_wide() {
        if escape {
            if x == BACKSLASH {
                backslashes += 1;
            } else {
                if x == QUOTE {
                    // Add n+1 backslashes to total 2n+1 before internal '"'.
                    cmdline.extend((0..=backslashes).map(|_| BACKSLASH));
                }
                backslashes = 0;
            }
        }
        cmdline.push(x);
    }
    if quote {
        // Add n backslashes to total 2n before ending '"'.
        cmdline.extend((0..backslashes).map(|_| BACKSLASH));
        cmdline.push('"' as u16);
    }

    Ok(())
}

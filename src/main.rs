//===- main.rs ------------------------------------------------------------===//
//
// Part of the Project “create-process-rust”, licensed under the Apache License,
// Version 2.0.  See the file “LICENSE.txt” in the root of this repository for
// license information.
//
// SPDX-License-Identifier: Apache-2.0
//
// © Copyright Contributors to the Rust Project
// © Copyright 2023 Jan Ole Hüser
//
// This file containes copied and modified source code of the Rust project, as
// described in the file “LICENSE.txt”.
//
//===----------------------------------------------------------------------===//


mod input;
mod output;
mod commandline;
mod process;
mod options;

use std::fs::File;
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

use itertools::Itertools;
use windows::Win32::System::Environment;

use serde::{
    Deserialize,
    Serializer,
    ser::SerializeStruct,
};

use base64::Engine as _;

struct Arg<'lifetime_of_slice> {
    arg: OsString,
    range: std::ops::Range<usize>,
    raw: &'lifetime_of_slice[u16],
    number: usize,
}

impl<'lifetime_of_slice> Arg<'lifetime_of_slice> {
    fn write_pretty_json_to_writer<W>(self: &Self, mut writer: &mut W, indent: &str) -> io::Result<()>
    where
        W: io::Write + ?Sized,
    {
        let (arg_lossy, arg) = match self.arg.to_str() {
            Some(inner) => (false, std::borrow::Cow::from(inner)),
            None      => (true, self.arg.to_string_lossy()),
        };
        let raw_os_str = OsString::from_wide(self.raw);
        let (raw_lossy, raw) = match raw_os_str.to_str() {
            Some(inner) => (false, std::borrow::Cow::from(inner)),
            None      => (true, self.arg.to_string_lossy()),
        };
        let arg_vec : Vec<u16> = self.arg.encode_wide().collect();


        if false {
            write!(&mut writer, "{indent}{{\n{indent}  \"arg\": ", indent = &indent)?;
            serde_json::to_writer(&mut writer, &serde_json::json!(arg))?;

            write!(&mut writer, ",\n{indent}  \"raw\": ", indent = &indent)?;
            serde_json::to_writer(&mut writer, &serde_json::json!(raw))?;

            write!(&mut writer, ",\n{indent}  \"arg-utf16\": ", indent = &indent)?;
            serde_json::to_writer(&mut writer, &serde_json::json!(arg_vec))?;

            write!(&mut writer, ",\n{indent}  \"raw-utf16\": ", indent = &indent)?;
            serde_json::to_writer(&mut writer, &serde_json::json!(self.raw))?;

            write!(&mut writer, ",\n{indent}  \"arg-lossy\": ", indent = &indent)?;
            serde_json::to_writer(&mut writer, &serde_json::json!(arg_lossy))?;

            write!(&mut writer, ",\n{indent}  \"raw-lossy\": ", indent = &indent)?;
            serde_json::to_writer(&mut writer, &serde_json::json!(raw_lossy))?;

            write!(&mut writer, ",\n{indent}  \"raw-start\": ", indent = &indent)?;
            serde_json::to_writer(&mut writer, &serde_json::json!(self.range.start))?;

            write!(&mut writer, ",\n{indent}  \"raw-end\": ", indent = &indent)?;
            serde_json::to_writer(&mut writer, &serde_json::json!(self.range.start))?;

            write!(&mut writer, ",\n{indent}}}", indent = &indent)?;
        }
        else {
            write!(&mut writer,
"{indent}{{
{indent}  \"arg\": {},
{indent}  \"raw\": {},
{indent}  \"arg-utf16\": {},
{indent}  \"raw-utf16\": {},
{indent}  \"arg-lossy\": {},
{indent}  \"raw-lossy\": {},
{indent}  \"raw-start\": {},
{indent}  \"raw-end\": {}
{indent}}}",
                &serde_json::json!(arg),
                &serde_json::json!(raw),
                &serde_json::json!(arg_vec),
                &serde_json::json!(self.raw),
                &serde_json::json!(arg_lossy),
                &serde_json::json!(raw_lossy),
                &serde_json::json!(self.range.start),
                &serde_json::json!(self.range.end),
                indent = &indent
            )?;
        }

        Ok(())
    }
}

impl<'lifetime_of_slice> serde::Serialize for Arg<'lifetime_of_slice> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let (arg_lossy, arg) = match self.arg.to_str() {
            Some(inner) => (false, std::borrow::Cow::from(inner)),
            None      => (true, self.arg.to_string_lossy()),
        };
        let raw_os_str = OsString::from_wide(self.raw);
        let (raw_lossy, raw) = match raw_os_str.to_str() {
            Some(inner) => (false, std::borrow::Cow::from(inner)),
            None      => (true, self.arg.to_string_lossy()),
        };
        //let arg_vec : Vec<u16> = self.arg.encode_wide().collect();
        // 3 is the number of fields in the struct.
        let mut state = serializer.serialize_struct("Arg", 6)?;
        state.serialize_field("arg", &arg)?;
        state.serialize_field("arg-lossy", &arg_lossy)?;
        //state.serialize_field("arg-utf16", &arg_vec)?;
        state.serialize_field("raw", &raw)?;
        state.serialize_field("raw-lossy", &raw_lossy)?;
        //state.serialize_field("raw-utf16", &self.raw)?;
        state.serialize_field("raw-start", &self.range.start)?;
        state.serialize_field("raw-end", &self.range.end)?;
        state.end()
    }
}

impl fmt::Display for Arg<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (lossless_or_lossy, arg) = match self.arg.to_str() {
            Some(arg) => ("lossless:", std::borrow::Cow::from(arg)),
            None      => ("lossy:   ", self.arg.to_string_lossy()),
        };
        let raw = OsString::from_wide(self.raw);
        let raw = raw.to_string_lossy();
        write!(f, "Argument {}, range {}..{}, {} »{}«, raw: »{}«",
                 self.number, self.range.start, self.range.end, lossless_or_lossy, arg, raw)
    }
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

fn get_command_line() -> Result<&'static [u16], &'static str> {
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

#[derive(Debug)]
enum ProgramOpt{
    Str(OsString),
    Null,
    FromCmdLine,
    FromJSONFile(OsString), // filename
}

#[derive(Debug)]
enum CmdlineOpt{
    Str(OsString),
    Null,
    FromJSONFile(OsString), // filename
}

#[derive(Debug)]
struct ExecOptions{
    program : ProgramOpt,
    cmdline : CmdlineOpt,
    prepend_program : bool,
    strip_program : bool,
    dry_run : bool,
    split_and_print_inner_cmdline: bool,
}

struct PrintOptions{
    json : bool,
    silent : bool,
    print_args : bool,
}

#[derive(Debug)]
enum MainChoice{
    Help,
    PrintArgs,
    ExecOpts(ExecOptions),
}

struct MainOptions{
    print_opts : PrintOptions,
    main_choice : MainChoice,
}

fn print_usage<W>(arg0 : &str, mut writer : &mut W)
    -> io::Result<()>
where
    W: io::Write + ?Sized
{
    let dirty_text = if env!("GIT_DIRTY") == "true" {"(working tree dirty)"}else {""};
    writeln!(&mut writer, "
create-process-rust, version {1} {2}

USAGE:
  \"{0}\" [<PRINT_OPTION>...] [--print-args-only <arg>...]

  \"{0}\" {{ --help | -help | /help | -h | /h | -? | /? }}

  \"{0}\"
    [<PRINT_OPTION>...]
    [--print-args]
    [--dry-run]
    [--split-and-print-inner-cmdline]
    {{
      {{ {{ --program <program> | --program-utf16le-base64 <encoded-program> }} [--prepend-program] }} |
      {{ --program-from-cmd-line [--strip-program] }} |
      --program-is-null
    }}
    {{
      --cmd-line-is-null |
      --cmd-line-in-arg <cmdline> |
      --cmd-line-utf16le-base64 <encoded-cmd-line> |
      --cmd-line-from-json-array <file> |
      --cmd-line-is-rest <arg>...
    }}


DESCRIPTION:

  Create a process by calling the Windows function `CreateProcessW`.

  Note: `<arg>...` in the USAGE section means that all following arguments are consumed. That means, they don't get interpreted as options.


OPTIONS:

  --help, -help, /help, -h, /h, -?, /?
    Print this help text.

  --print-args
    Print all the arguments to this program.

  --dry-run
    Don’t actually execute the program.

  --split-and-print-inner-cmdline
    Split the command line assembled from the value of a `--cmd-line-*` option and other options into arguments and print those arguments.

  --print-args-only
    Print all arguments to this program and do nothing else.

  --program <program>
    Specify the path to the program to start.

  --program-utf16le-base64 <encoded-program>
    Specify the path to the program to start as an base64-encoded UTF-16 little endian string.

  --prepend-program
    Prepend the program to the command line.
    This is only valid with `--program* <program>`, where `*` can be the empty string or `-utf16le-base64`.

  --program-from-cmd-line
    Parse the program from the command line given by a `--cmd-line-*` option.

  --strip-program
    Strip the program from the command line given by a `--cmd-line-*` option.
    This is only valid with `--program-from-cmd-line`.

  --program-is-null
    The first argument to CreateProcessW is NULL.

  --cmd-line-is-null
    The second argument to CreateProcessW is NULL.

  --cmd-line-in-arg <cmdline>
    Specify the command line in one argument.

  --cmd-line-utf16le-base64 <encoded-cmd-line>
    Specify the command line as an base64-encoded UTF-16 little endian string.

  --cmd-line-from-json <file>
    Read the command line from a JSON file. Write a dash/hyphen (-) for stdin.

  --cmd-line-is-rest <arg>...
    Use the rest of the command line as new command line.


PRINT_OPTIONS:

  --json
    Output Data as JSON

  --silent
    Don't be verbose


", arg0, env!("GIT_HASH"), dirty_text)
}

fn get_rest<'a>(cmd_line:&'a[u16], arg: &Arg<'a>) -> &'a[u16]{
    let start_of_rest = arg.range.end + 1;
    match cmd_line.get(start_of_rest) {
        Some(_) => {
            &cmd_line[start_of_rest..]
        },
        None => &[],
    }
}

fn decode_utf16le_base64(input: &OsStr) -> Result<OsString,String> {
    match input.to_str() {
        None => Err("cannot convert to UTF-8".to_string()),
        Some(utf8) => {
            let decode_result = base64::engine::general_purpose::STANDARD.decode(utf8);
            match decode_result {
                Err(_) => Err("cannot interpret input as base64".to_string()),
                Ok(vec) => {
                    if vec.len() % 2 != 0 {
                        return Err("decoded base64 does not result in an even number of bytes".to_string());
                    }
                    let mut vec16 : Vec<u16> = std::vec::Vec::with_capacity(vec.len()/2);
                    let mut uint16: u16 = 0u16;
                    let mut even : bool = true;
                    // I don't know, if I do it correctly for little endian, but we'll see.
                    // I think, we must respect the byte ordering of the CPU architecture.
                    // Therefor we must call a function to get the order.
                    for uint8 in vec {
                        if even {
                            uint16 = uint8.into();
                        } else {
                            uint16 = uint16 | u16::from(uint8).overflowing_shl(8).0;
                            vec16.push(uint16)
                        }
                        even = !even;
                    }
                    Ok(OsString::from_wide(&vec16[..]))
                }
            }
        }
    }

}

fn get_options(cmd_line : &[u16], args: &Vec<Arg>) -> Result<MainOptions,String> {
    let mut args_iter = args.iter();
    let mut print_opts = PrintOptions{
        print_args: false,
        json: false,
        silent: false,
    };

    // skip first/zerothed argument
    if let None = args_iter.next() {
        print_opts.print_args = true;
        return Ok( MainOptions{ print_opts, main_choice: MainChoice::PrintArgs, });
    }

    let opt_program : &OsStr = OsStr::new("--program");
    let opt_program_utf16le_base64 : &OsStr = OsStr::new("--program-utf16le-base64");
    let opt_program_from_cmd_line : &OsStr = OsStr::new("--program-from-cmd-line");
    let opt_program_is_null : &OsStr = OsStr::new("--program-is-null");
    let opt_cmd_line_in_arg : &OsStr = OsStr::new("--cmd-line-in-arg");
    let opt_cmd_line_utf16le_base64 : &OsStr = OsStr::new("--cmd-line-utf16le-base64");
    let opt_cmd_line_is_rest : &OsStr = OsStr::new("--cmd-line-is-rest");
    let opt_cmd_line_is_null : &OsStr = OsStr::new("--cmd-line-is-null");
    let opt_prepend_program : &OsStr = OsStr::new("--prepend-program");
    let opt_strip_program : &OsStr = OsStr::new("--strip-program");
    let opt_split_and_print_inner_cmdline : &OsStr = OsStr::new("--split-and-print-inner-cmdline");
    let opts_help : Vec<&OsStr> = vec![
        OsStr::new("--help"),
        OsStr::new("-help"),
        OsStr::new("/help"),
        OsStr::new("-h"),
        OsStr::new("/h"),
        OsStr::new("-?"),
        OsStr::new("/?"),
    ];
    let opt_print_args : &OsStr = OsStr::new("--print-args");
    let opt_print_args_only : &OsStr = OsStr::new("--print-args-only");
    let opt_dry_run : &OsStr = OsStr::new("--dry-run");
    let opt_json : &OsStr = OsStr::new("--json");
    let opt_silent : &OsStr = OsStr::new("--silent");

    let mut program : Option<ProgramOpt> = None;
    let mut cmdline_opt : Option<CmdlineOpt> = None;
    let mut prepend_program : bool = false;
    let mut strip_program : bool = false;
    let mut dry_run : bool = false;
    let mut split_and_print_inner_cmdline = false;

    let mut only_print_opts_thus_far = true;
    while let Some(arg) = args_iter.next() {
        match arg.arg.as_os_str() {
            x if x == opt_dry_run => {
                dry_run = true;
            }
            x if x == opt_prepend_program => {
                prepend_program = true;
            },
            x if x == opt_strip_program => {
                strip_program = true;
            },
            x if x == opt_split_and_print_inner_cmdline => {
                split_and_print_inner_cmdline = true;
            },
            x if x == opt_print_args => {
                print_opts.print_args = true;
                continue; // skip setting only_print_opts_thus_far to false
            },
            x if x == opt_json => {
                print_opts.json = true;
                continue; // skip setting only_print_opts_thus_far to false
            },
            x if x == opt_silent => {
                print_opts.silent = true;
                continue; // skip setting only_print_opts_thus_far to false
            },
            x if x == opt_print_args_only => {
                return if only_print_opts_thus_far {
                    print_opts.print_args = true;
                    return Ok( MainOptions{ print_opts, main_choice: MainChoice::PrintArgs, });
                } else {
                    Err(format!("bad option, \"{}\" may only be the first argument:\n  {}",
                                &opt_cmd_line_in_arg.to_string_lossy(),
                                &arg))
                };
            }
            x if opts_help.contains(&x) => {
                return Ok( MainOptions{ print_opts, main_choice: MainChoice::Help, });
            },
            x if x == opt_program => {
                if program.is_some() {
                    return Err(format!("bad option, program is already initilaized:\n  {}", &arg));
                }
                match args_iter.next() {
                    Some(next_arg) => program = Some(ProgramOpt::Str(next_arg.arg.clone())),
                    None => return Err(format!("missing argument for option:\n  {}", &arg)),
                }
            },
            x if x == opt_program_utf16le_base64 => {
                if program.is_some() {
                    return Err(format!("bad option, program is already initilaized:\n  {}", &arg));
                }
                match args_iter.next() {
                    Some(next_arg) => {
                        match decode_utf16le_base64(&next_arg.arg) {
                            Ok(p) => program = Some(ProgramOpt::Str(p)),
                            Err(err_str) => return Err(format!("bad argument for the following option: {}\n {}\nbad argument:\n {}", &err_str, &arg, &next_arg)),
                        }
                    },
                    None => return Err(format!("missing argument for option:\n  {}", &arg)),
                }
            },
            x if x == opt_program_from_cmd_line => {
                if program.is_some() {
                    return Err(format!("bad option, program is already initilaized:\n  {}", &arg));
                }
                program = Some(ProgramOpt::FromCmdLine);
            },
            x if x == opt_program_is_null => {
                if program.is_some() {
                    return Err(format!("bad option, program is already initilaized:\n  {}", &arg));
                }
                program = Some(ProgramOpt::Null);
            },
            x if x == opt_cmd_line_in_arg => {
                if cmdline_opt.is_some() {
                    return Err(format!("bad option, cmd line is already initilaized:\n  {}", &arg));
                }
                match args_iter.next() {
                    Some(next_arg) => cmdline_opt = Some(CmdlineOpt::Str(next_arg.arg.clone())),
                    None => return Err(format!("missing argument for option:\n  {}", &arg)),
                }
            },
            x if x == opt_cmd_line_utf16le_base64 => {
                if cmdline_opt.is_some() {
                    return Err(format!("bad option, cmd line is already initilaized:\n  {}", &arg));
                }
                match args_iter.next() {
                    Some(next_arg) => {
                        match decode_utf16le_base64(&next_arg.arg) {
                            Ok(p) => cmdline_opt = Some(CmdlineOpt::Str(p)),
                            Err(err_str) => return Err(format!("bad argument for the following option: {}\n {}\nbad argument:\n {}", &err_str, &arg, &next_arg)),
                        }
                    },
                    None => return Err(format!("missing argument for option:\n  {}", &arg)),
                }
            },
            x if x == opt_cmd_line_is_rest => {
                if cmdline_opt.is_some() {
                    return Err(format!("bad option, cmd line is already initilaized:\n  {}", &arg));
                }
                cmdline_opt = Some(CmdlineOpt::Str(OsString::from_wide(get_rest(cmd_line, arg))));
                break; // break, because all args get consumed
            },
            x if x == opt_cmd_line_is_null => {
                if cmdline_opt.is_some() {
                    return Err(format!("bad option, cmd line is already initilaized:\n  {}", &arg));
                }
                cmdline_opt = Some(CmdlineOpt::Null);
            },
            _other => {
                return Err(format!("unknown option:\n  {}", &arg));
            }
        }
        only_print_opts_thus_far = false;
    }
    match (program, cmdline_opt, print_opts.print_args, only_print_opts_thus_far) {
        (None, None, true, _) => Ok(MainOptions{ print_opts, main_choice: MainChoice::PrintArgs, }),
        (None, None, _, true) => Ok(MainOptions{ print_opts, main_choice: MainChoice::PrintArgs, }),
        (None, None, _, _) => Err("Neither program nor cmd line were specified".to_owned()),
        (None, _, _, _) => Err("program was not specied".to_owned()),
        (_, None, _, _) => Err("cmd line was not specied".to_owned()),
        (Some(program), Some(cmdline), _, _) =>
            Ok(
                MainOptions{
                    print_opts,
                    main_choice : MainChoice::ExecOpts(
                        ExecOptions{ program, cmdline, prepend_program,
                                     strip_program, dry_run, split_and_print_inner_cmdline }
                    )
                }
            ),
    }
}

fn print_args<W>(
    cmdline: &[u16],
    parsed_args_list: &Vec<Arg<'_>>,
    print_opts: &PrintOptions,
    indent: &str,
    print_header: bool,
    mut writer: &mut W
) -> io::Result<()>
where
    W: io::Write + ?Sized
{
    let cmdline_os_string : OsString = OsStringExt::from_wide(cmdline);
    let (cmdline_lossy, cmdline_utf8) = match cmdline_os_string.to_str() {
        Some(str) => {
            (false, std::borrow::Cow::from(str))
        },
        None => {
            (true, cmdline_os_string.to_string_lossy())
        }
    };
    if print_opts.json {
        let mut stdout = io::stdout().lock();
        write!(stdout,
"{indent}{{
{indent}  \"cmdline\": {},
{indent}  \"cmdline-utf16\": {},
{indent}  \"cmdline-lossy\": {},
{indent}  \"args\": [
",
            serde_json::json!(cmdline_utf8),
            serde_json::json!(cmdline),
            serde_json::json!(cmdline_lossy),
            indent = indent
        )?;

        let mut first = true;
        for x in parsed_args_list {
            if first {
                first = false
            } else {
                stdout.write_all(b",\n")?;
            }
            x.write_pretty_json_to_writer(&mut stdout, &(indent.to_owned() + "    "))?;
        }
        write!(stdout,"\n{indent}  ]\n{indent}}}\n", indent = indent)?;
    }
    else {
        // TODO: privide info about lossy or lossless
        if print_header {
            writeln!(&mut writer, "The command line is put in quotes (»«). \
                     If those quotes are inside the command line, they are not escaped. \
                     The command line is: \n\
                     »{}«\n", cmdline_utf8)?;
        }
        let mut n : usize = 0;
        for Arg {arg, range, raw, ..} in parsed_args_list {
            let (lossless_or_lossy, arg) = match arg.to_str() {
                Some(arg) => ("lossless:", std::borrow::Cow::from(arg)),
                None      => ("lossy:   ", arg.to_string_lossy()),
            };
            let raw = OsString::from_wide(raw);
            let raw = raw.to_string_lossy();
            writeln!(&mut writer, "Argument {:2}, {:3} .. {:3}, {} »{}«, raw: »{}«",
                     n, range.start, range.end, lossless_or_lossy, arg, raw)?;
            n += 1;
        }
    }
    Ok(())
}

fn quote_or_null
    <S: AsRef<OsStr>>
    (opt: Option<S>)
    -> Cow<'static, str>
{
    match opt {
        None => Cow::from("NULL"),
        Some(os_str) => Cow::from(format!("»{}«",os_str.as_ref().to_string_lossy())),
    }
}



enum StdOutOrStdErr{
    StdOut(std::io::Stdout),
    StdErr(std::io::Stderr),
}

impl StdOutOrStdErr{
    fn into_writer(&mut self) -> &mut dyn io::Write {
        match self {
            StdOutOrStdErr::StdOut(stdout) => stdout,
            StdOutOrStdErr::StdErr(stderr) => stderr,
        }
    }
}

fn main() -> Result<(), String>{
    let cmdline: &'static [u16] = get_command_line()?;
    let parsed_args_list : Vec<Arg<'static>> = parse_lp_cmd_line(cmdline, true);
    let arg0_or_default : std::borrow::Cow<'_, str> = match parsed_args_list.first() {
        Some(arg) => arg.arg.to_string_lossy(),
        None => std::borrow::Cow::from("create-process-rust"),
    };

    let options : MainOptions = match get_options(cmdline, &parsed_args_list){
        Ok(options) => options,
        Err(msg) => {
            eprintln!("{}\n",msg);
            print_args(cmdline, &parsed_args_list,
                       &PrintOptions { json: false, silent: false, print_args: true },
                       "", true, &mut std::io::stderr())
                .map_err(|error| error.to_string())?;

            return Err("bad option".to_owned());
        },
    };

    match options.main_choice {
        MainChoice::PrintArgs => {
            print_args(cmdline, &parsed_args_list, & options.print_opts, "", true, &mut std::io::stdout()).map_err(|error| error.to_string())
        },
        MainChoice::Help => {
            print_usage(&arg0_or_default, &mut std::io::stdout()).map_err(|x| format!("Print usage failed with: {}", x.to_string()))
        },
        MainChoice::ExecOpts(opts) => {
            exec(opts, options.print_opts, cmdline, parsed_args_list)
        },
    }
}

struct EscapedArgZero<'a>{
    escaped : Cow<'a, [u16]>,
    warning: Option<&'static str>,
}

fn escape_arg_zero<'a>(slice_utf16: &'a [u16], force_quotes: bool)-> Result<EscapedArgZero<'a>, String> {
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

fn print_inner_cmdline(cmdline_opt: &Option<OsString>, print_opts: &PrintOptions) -> Result<(), String> {
    match &cmdline_opt {
        Some(cmdline_str)  => {
            let cmdline_vec = cmdline_str.encode_wide().collect_vec();
            let cmdline_slice = &cmdline_vec[..];
            let inner_parsed_args_list = parse_lp_cmd_line(cmdline_slice, true);
            print_args(cmdline_slice, &inner_parsed_args_list, &print_opts, "", false, &mut std::io::stdout())
                .map_err(|error| error.to_string())?;
        },
        _ => {}
    };
    Ok(())
}

#[derive(Deserialize, Debug)]
struct JsonUserInput {
    args: Option<Vec<String>>,
    cmdline: Option<String>,
    program: Option<String>,
}

enum StdInOrBufReader{
    StdIn(std::io::StdinLock<'static>),
    BufReader(std::io::BufReader<File>),
}

impl StdInOrBufReader{
    fn into_writer(&mut self) -> &mut dyn io::Read {
        match self {
            StdInOrBufReader::StdIn(stdin) => stdin,
            StdInOrBufReader::BufReader(bufreader) => bufreader,
        }
    }
}

fn is_filename_stdin(file_name : &OsStr) -> bool {
    file_name == OsStr::new("-")
}

fn read_user_input_from_file(file : &OsStr) -> Result<JsonUserInput, String> {
    let mut reader = if is_filename_stdin(file) {
        StdInOrBufReader::StdIn(io::stdin().lock())
    } else {
        let file = File::open(file).map_err(|error| error.to_string())?;
        let buf_reader = std::io::BufReader::new(file);
        StdInOrBufReader::BufReader(buf_reader)
    };
    let mut de = serde_json::Deserializer::from_reader(reader.into_writer());
    let user_input = JsonUserInput::deserialize(&mut de).map_err(|error| error.to_string())?;

    Ok(user_input)
}

fn ensure_no_nuls<O: AsRef<OsStr>>(os_str: O) -> Result<(), String>{
    let os_str : &OsStr = os_str.as_ref();
    if os_str.encode_wide().any(|u| u == 0u16) {
        return Err("OsStr contains a NUL character".to_owned());
    }
    Ok(())
}

fn append_arg<O: AsRef<OsStr>>(cmdline: &mut Vec<u16>, arg: O, force_quotes: bool, raw: bool) -> Result<(), String> {
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

fn get_cmdline_from_args<'a, I, S>(args : I) -> String
where
    S : AsRef<str> + 'a ,
    I : std::iter::Iterator<Item = S>
{
    let result : String = String::new();
    for arg in args {
        let s : &str = arg.as_ref();
        println!("DEBUG: {}", s);
    }
    return result;
}

fn get_cmdline_from_json(json_user_input : &JsonUserInput) -> Result<OsString,String> {
    if json_user_input.cmdline.is_some() && json_user_input.args.is_some() {
        return Err("Do not provide \"args\" and \"cmdline\" in JSON".to_owned());
    }

    if let Some(cmdline) = &json_user_input.cmdline {
        Ok(OsString::from(cmdline))
    }else if let Some(args) = &json_user_input.args {
        let cmdline : String = get_cmdline_from_args(args.iter());
        Ok(OsStr::new("").to_owned())
    }
    else{
        Err("".to_owned())
    }
}
fn get_program_from_json(json_user_input : &JsonUserInput) -> Result<OsString,String> {

    return Err("TEMP TODO".to_owned());
}

fn exec(
    exec_options: ExecOptions,
    print_opts: PrintOptions,
    cmdline: &'static [u16],
    parsed_args_list : Vec<Arg<'static>>,
)
    -> Result<(), String>
{

    if print_opts.print_args {
        print_args(cmdline, &parsed_args_list, &print_opts, "", true, &mut std::io::stdout())
            .map_err(|error| error.to_string())?;
    }

    if exec_options.strip_program
        && (match exec_options.program { ProgramOpt::FromCmdLine => false, _ => true })
    {
        return Err("Error: \"--strip-program\" can only be specified with \
                   \"--program-from-cmd-line\".".to_owned());
    }

    if exec_options.prepend_program
        && (match &exec_options.program { ProgramOpt::FromCmdLine => true, ProgramOpt::Null => true, _ => false }) {
        return Err("Error: \"--prepend-program\" can not be combined with \
                   \"--program-from-cmd-line\" or \"--program-is-null\".".to_owned());
    }

    let cmdline_comes_from_stdin =
        if let CmdlineOpt::FromJSONFile(file_name) = &exec_options.cmdline{
            is_filename_stdin(file_name)
        }
        else {false};
    let program_comes_from_stdin =
        if let ProgramOpt::FromJSONFile(file_name) = &exec_options.program{
            is_filename_stdin(file_name)
        }
        else {false};

    let json_user_input_from_stdin : Option<JsonUserInput> =
        if cmdline_comes_from_stdin || program_comes_from_stdin {
            Some(read_user_input_from_file(OsStr::new("-"))?)
        } else {
            None
        };

    let mut new_cmdline : Option<OsString> = match exec_options.cmdline {
        CmdlineOpt::Str(os_str) => Some(os_str),
        CmdlineOpt::Null => None,
        CmdlineOpt::FromJSONFile(file_name) => {
            Some(
                if cmdline_comes_from_stdin { get_cmdline_from_json((&json_user_input_from_stdin).as_ref().unwrap())? }
                else { get_cmdline_from_json(&read_user_input_from_file(&file_name)?)? }
            )
        },
    };


    let program: Option<Cow<'_,OsStr>> = match exec_options.program {
        ProgramOpt::Null => {
            None
        },
        ProgramOpt::FromJSONFile(file_name) => {
            let program_from_json : OsString =
                if program_comes_from_stdin {
                    get_program_from_json((&json_user_input_from_stdin).as_ref().unwrap())? }
                else {
                    get_program_from_json(&read_user_input_from_file(&file_name)?)?
                };
            Some(Cow::from(program_from_json))
        }
        ProgramOpt::Str(prog) => {
            Some(Cow::from(prog))
        },
        ProgramOpt::FromCmdLine => {
            let cmdline_os_str: &OsStr = match &new_cmdline{
                None => return Err("Error: cannot get program from cmd line, because cmd line is NULL.".to_owned()),
                Some(cmdline_str) => cmdline_str.as_os_str(),
            };
            let x = OsStrExt::encode_wide(cmdline_os_str);
            let new_cmdline_u16 :Vec<u16> = x.collect();
            let new_parsed_args = parse_lp_cmd_line(&new_cmdline_u16, false);
            match new_parsed_args.into_iter().next() {
                Some(arg) => {
                    if exec_options.strip_program {
                        new_cmdline = Some(OsString::from_wide(get_rest(&new_cmdline_u16, &arg)));
                    }
                    Some(Cow::from(arg.arg))
                },
                None => {
                    return Err("Error: Couldn't get program from command line".to_owned());
                },
            }
        },
    };

    match &program {
        Some(cow_prog) => 
            if exec_options.prepend_program {
                match &new_cmdline {
                    &None => return Err("Cannot prepend program to cmdline, if cmdline is NULL.".to_owned()),
                    &Some(ref old_cmd) => {
                        let prog_vec: Vec<u16> = cow_prog.encode_wide().collect();
                        let escaped_arg_zero = escape_arg_zero(&prog_vec, false)?;
                        if let Some(warning) = escaped_arg_zero.warning {
                            eprintln!("Warning: {}", warning);
                        }
                        let mut new_cmd = OsString::from_wide(&escaped_arg_zero.escaped);
                        new_cmd.push(OsString::from(" "));
                        new_cmd.push(OsString::from(old_cmd));
                        new_cmdline = Some(new_cmd);
                    },
                }
            }
        None => {}
    };


    let mut writer_wrapper: StdOutOrStdErr =
        if print_opts.print_args || exec_options.split_and_print_inner_cmdline { StdOutOrStdErr::StdErr(io::stderr())}
        else { StdOutOrStdErr::StdOut(io::stdout()) };

    // from https://doc.rust-lang.org/std/option/ :
    //   as_deref converts from &Option<T> to Option<&T::Target>
    //
    // `T` in this case is `OsString` and `T::Target` should be `OsStr` or `&OsStr`.
    writeln!(&mut writer_wrapper.into_writer(), "The program      (1st argument to CreateProcessW) is:   {}", quote_or_null((&program).as_deref()))
        .map_err(|x| format!("Write failed with {}", x.to_string()))?;
    writeln!(
             &mut writer_wrapper.into_writer(),
             "The command line (2nd argument to CreateProcessW) is:   {}",
             quote_or_null(new_cmdline.as_deref())
        ).map_err(|x| format!("Write failed with {}", x.to_string()))?;

    if exec_options.split_and_print_inner_cmdline {
        print_inner_cmdline(&new_cmdline,&print_opts)?;
    }

    writeln!(&mut writer_wrapper.into_writer(), "Execute process:\n")
        .map_err(|x| format!("Write failed with {}", x.to_string()))?;

    let exit_code : u32 =
        if ! exec_options.dry_run {
            crate::process::create_process(program.as_deref(), new_cmdline.as_deref())?
        } else {
            writeln!(&mut writer_wrapper.into_writer(), "\nSkipping execution because of --dry-run.")
                .map_err(|x| format!("Write failed with {}", x.to_string()))?;
            0
        };

    writeln!(&mut writer_wrapper.into_writer(), "\nThe exit code is {}", exit_code)
        .map_err(|x| format!("Write failed with {}", x.to_string()))?;


    if false {
        loop {
            write!(&mut writer_wrapper.into_writer(), ".").unwrap();
            io::stdout().flush().map_err(|_| "Failed to flush stdout")?;
            thread::sleep(time::Duration::from_millis(2000));
        }
    }
    std::process::exit(exit_code as i32);
}


// What commandline options do I want to have?
// We do these:
// --print-args
// --program <program>
// --program=<program>               // not supported right now
// --program-is-null
// --program-from-cmd-line
// --cmd-line-in-arg <commandline>
// --cmd-line-in-arg=<commandline>   // not supported right now
// --cmd-line-is-rest <args>...
// --cmd-line-is-null
// --prepend-program
// --strip-program
// --json                            // output as json
//
// For later
// --cmd-line-from-stdin
// --handle-first-in-rest-special    // handle the first arg in `<arg>...` with the special rule for argument zero
// --only-print-rest                 // only print those arguments that come in `<arg>...`
//
// Ideas:
// - dont do array of numbers for utf16 in JSON, but instead to base64
// - reinterpret the command line,
//   for example the raw argument »hello" "world« becomes »"hello world"« and »--peter="gustav"« loses it’s quotes an becomes »--peter=gustav«.
//   We could the also have --force-quotes
// - Resolve path of program using the environment variable PATH

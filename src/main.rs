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


use std::{
    borrow::Cow,
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

use serde::{
    Serialize,
    Serializer,
    ser::SerializeStruct,
};


struct Arg<'lifetime_of_slice> {
    arg: OsString,
    range: std::ops::Range<usize>,
    raw: &'lifetime_of_slice[u16],
    number: usize,
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
        let arg_vec : Vec<u16> = self.arg.encode_wide().collect();
        // 3 is the number of fields in the struct.
        let mut state = serializer.serialize_struct("Arg", 8)?;
        state.serialize_field("arg", &arg)?;
        state.serialize_field("arg-lossy", &arg_lossy)?;
        state.serialize_field("arg-utf16", &arg_vec)?;
        state.serialize_field("raw", &raw)?;
        state.serialize_field("raw-lossy", &raw_lossy)?;
        state.serialize_field("raw-utf16", &self.raw)?;
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
}

#[derive(Debug)]
struct ExecOptions{
    program : ProgramOpt,
    cmdline : Option<OsString>,
    prepend_program : bool,
    strip_program : bool,
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

fn print_usage(arg0 : &str) {
    println!("
USAGE:
  \"{0}\" [<PRINT_OPTION>...] [--print-args-only <arg>...]

  \"{0}\" {{ --help | -help | /help | -h | /h | -? | /? }}

  \"{0}\"
    [<PRINT_OPTION>...]
    [--print-args]
    {{
      {{ --program <program> [--prepend-program] }} |
      {{ --program-from-cmd-line [--strip-program] }} |
      --program-is-null
    }}
    {{
      --cmd-line-in-arg <cmdline> |
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

  --print-args-only
    Print all arguments to this program and do nothing else.

  --program <program>
    Specify the path to the program to start.

  --prepend-program
    Prepend the program to the command line.
    This is only valid with `--program <program>`.
    This is not supported right now.

  --program-from-cmd-line
    Parse the program from the command line given by a `--cmd-line-*` option.

  --strip-program
    Strip the program from the command line given by a `--cmd-line-*` option.
    This is only valid with `--program-from-cmd-line`.

  --program-is-null
    The first argument to CreateProcessW is NULL.

  --cmd-line-in-arg <cmdline>
    Specify the command line in one argument.

  --cmd-line-is-rest <arg>...
    Use the rest of the command line as new command line.


PRINT_OPTIONS:
 
  --json
    Output Data as JSON

  --silent
    Don't be verbose
  

", arg0);
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
    let opt_program_from_cmd_line : &OsStr = OsStr::new("--program-from-cmd-line");
    let opt_program_is_null : &OsStr = OsStr::new("--program-is-null");
    let opt_cmd_line_in_arg : &OsStr = OsStr::new("--cmd-line-in-arg");
    let opt_cmd_line_is_rest : &OsStr = OsStr::new("--cmd-line-is-rest");
    let opt_cmd_line_is_null : &OsStr = OsStr::new("--cmd-line-is-null");
    let opt_prepend_program : &OsStr = OsStr::new("--prepend-program");
    let opt_strip_program : &OsStr = OsStr::new("--strip-program");
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
    let opt_json : &OsStr = OsStr::new("--json");
    let opt_silent : &OsStr = OsStr::new("--silent");

    let mut program : Option<ProgramOpt> = None;
    let mut cmdline_opt : Option<Option<OsString>> = None;
    let mut prepend_program : bool = false;
    let mut strip_program : bool = false;

    let mut only_print_opts_thus_far = true;
    while let Some(arg) = args_iter.next() {
        match arg.arg.as_os_str() {
            x if x == opt_prepend_program => {
                prepend_program = true;
            },
            x if x == opt_strip_program => {
                strip_program = true;
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
                    Some(next_arg) => cmdline_opt = Some(Some(next_arg.arg.clone())),
                    None => return Err(format!("missing argument for option:\n  {}", &arg)),
                }
            },
            x if x == opt_cmd_line_is_rest => {
                if cmdline_opt.is_some() {
                    return Err(format!("bad option, cmd line is already initilaized:\n  {}", &arg));
                }
                cmdline_opt = Some(Some(OsString::from_wide(get_rest(cmd_line, arg))));
                break; // break, because all args get consumed
            },
            x if x == opt_cmd_line_is_null => {
                if cmdline_opt.is_some() {
                    return Err(format!("bad option, cmd line is already initilaized:\n  {}", &arg));
                }
                cmdline_opt = Some(None);
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
                        ExecOptions{ program, cmdline, prepend_program, strip_program, }
                    )
                }
            ),
    }
}

fn print_args(cmdline: &[u16], parsed_args_list: &Vec<Arg<'_>>, print_opts: &PrintOptions, indent: &str) -> io::Result<()>{
    let cmdline_os_string : OsString = OsStringExt::from_wide(cmdline);
    let cmdline_u8 = match cmdline_os_string.to_str() {
        Some(str) => {
            println!("The command line was converted losslessly.");
            std::borrow::Cow::from(str)
        },
        None => {
            println!("The command line was converted lossy!");
            cmdline_os_string.to_string_lossy()
        }
    };
    println!("The command line is put in quotes (»«). \
              If those quotes are inside the command line, they are not escaped. \
              The command line is: \n\
              »{}«\n", cmdline_u8);

    if print_opts.json {
        let mut stdout = io::stdout().lock();
        serde_json::to_writer_pretty(&mut stdout, parsed_args_list)?;
        stdout.write_all(b"\n")?;
    }
    else {
        let mut n : usize = 0;
        for Arg {arg, range, raw, ..} in parsed_args_list {
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
    }
    Ok(())
}

fn experiment_with_serde_json() -> io::Result<()>{
    let my_json = serde_json::json!({
        "command-line" : "\"hel\"lo world",
        "lossy": false,
        "command-line-utf-16": [1,2,3,4],
        "args": 
        [
            {
                "arg": "hello",
                "arg-lossy": false,
                "raw": "\"hel\"lo",
                "raw-lossy": false,
                "utf16": [1,2],
                "raw-utf16": [1,2],
            },
            {
                "arg": "world",
                "arg-raw": "world",
                "arg-utf-16": [3,4],
                "arg-raw-utf-16": [1,2],
            }
        ]
    });

    let mut stdout = io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &my_json)?;
    stdout.write_all(b"\n")?;
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

fn main() -> Result<(), String>{
    let cmdline: &'static [u16] = get_command_line()?;
    let parsed_args_list : Vec<Arg<'static>> = parse_lp_cmd_line(cmdline, true);
    let arg0_or_default : std::borrow::Cow<'_, str> = match parsed_args_list.first() {
        Some(arg) => arg.arg.to_string_lossy(),
        None => std::borrow::Cow::from("create-process-rust"),
    };

    //experiment_with_serde_json().map_err(|error| error.to_string())?;

    let options : MainOptions = match get_options(cmdline, &parsed_args_list){
        Ok(options) => options,
        Err(msg) => {
            println!("{}",msg);
            print_usage(&arg0_or_default);
            return Err("bad option".to_owned());
        },
    };

    let exec_options : ExecOptions = match options.main_choice {
        MainChoice::PrintArgs => {
            print_args(cmdline, &parsed_args_list, & options.print_opts, "").map_err(|error| error.to_string())?;
            return Ok(());
        },
        MainChoice::Help => {
            print_usage(&arg0_or_default);
            return Ok(());
        },
        MainChoice::ExecOpts(opts) => opts,
    };

    if options.print_opts.print_args {
        print_args(cmdline, &parsed_args_list, & options.print_opts, "").map_err(|error| error.to_string())?;
    }

    if exec_options.strip_program && (match exec_options.program { ProgramOpt::FromCmdLine => false, _ => true }) {
        return Err("Error: \"--strip-program\" can only be specified with \"--program-from-cmd-line\".".to_owned());
    }

    if exec_options.prepend_program && (match exec_options.program { ProgramOpt::Str(_) => false, _ => true }) {
        return Err("Error: \"--prepend-program\" can only be specified with \"--program\".".to_owned());
    }

    let mut new_cmdline : Option<OsString> = exec_options.cmdline;

    let program: Option<Cow<'_,OsStr>> = match exec_options.program {
        ProgramOpt::Null => {
            None
        },
        ProgramOpt::Str(str) => {
            if exec_options.prepend_program {
                return Err("Error: \"--prepend-program\" is not implemented yet".to_owned());
            }
            Some(Cow::from(str))
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

    // from https://doc.rust-lang.org/std/option/ :
    //   as_deref converts from &Option<T> to Option<&T::Target>
    //
    // `T` in this case is `OsString` and `T::Target` should be `OsStr` or `&OsStr`.

    println!("The program      (1st argument to CreateProcessW) is:   {}", quote_or_null((&program).as_deref()));
    println!("The command line (2nd argument to CreateProcessW) is:   {}", quote_or_null(new_cmdline.as_deref()));

    println!("Execute process:\n");
    let exit_code : u32 = create_process(program.as_deref(), new_cmdline.as_deref())?;
    println!("\nThe exit code is {}", exit_code);

    if false {
        loop {
            print!(".");
            io::stdout().flush().map_err(|_| "Failed to flush stdout")?;
            thread::sleep(time::Duration::from_millis(2000));
        }
    }
    std::process::exit(exit_code as i32);
}

fn create_process
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
            println!("Warning: Closing thread handle failed.");
        }
        process_information.hThread = HANDLE::default();
    }
    else {
        println!("Warning: Thread handle is invalid.");
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
//
// For later
// --cmd-line-from-stdin
// --json                            // output as json


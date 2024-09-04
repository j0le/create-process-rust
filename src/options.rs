use std::{
    borrow::Cow,
    convert::AsRef,
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
use base64::{engine::general_purpose::STANDARD as base64_STANDARD, Engine as _};

use crate::commandline;

#[derive(Debug)]
pub(super) enum ProgramOpt{
    Str(OsString),
    Null,
    FromCmdLine,
    FromJSONFile(OsString), // filename
}

#[derive(Debug)]
pub(super) enum CmdlineOpt{
    Str(OsString),
    Null,
    FromJSONFile(OsString), // filename
}

#[derive(Debug)]
pub(super) struct ExecOptions{
    pub(super) program : ProgramOpt,
    pub(super) cmdline : CmdlineOpt,
    pub(super) prepend_program : bool,
    pub(super) strip_program : bool,
    pub(super) dry_run : bool,
    pub(super) split_and_print_inner_cmdline: bool,
}

pub(super) struct PrintOptions{
    pub(super) json : bool,
    pub(super) silent : bool,
    pub(super) print_args : bool,
}

#[derive(Debug)]
pub(super) enum MainChoice{
    Help,
    PrintArgs,
    ExecOpts(ExecOptions),
}

pub(super) struct MainOptions{
    pub(super) print_opts : PrintOptions,
    pub(super) main_choice : MainChoice,
}

pub(super) fn print_usage<W>(arg0 : &str, mut writer : &mut W)
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


fn decode_utf16le_base64(input: &OsStr) -> Result<OsString,String> {
    match input.to_str() {
        None => Err("cannot convert to UTF-8".to_string()),
        Some(utf8) => {
            let decode_result = base64_STANDARD.decode(utf8);
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

pub(super) fn get_options(cmd_line : &[u16], args: &Vec<crate::commandline::Arg>) -> Result<MainOptions,String> {
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
                cmdline_opt = Some(CmdlineOpt::Str(OsString::from_wide(commandline::get_rest(cmd_line, arg))));
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



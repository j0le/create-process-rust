//===- main.rs ------------------------------------------------------------===//
//
// This file is part of the Project “create-process-rust”, licensed under 
// the Apache License, Version 2.0.  See the file “LICENSE.txt” in the root 
// of this repository for license information.
//
// SPDX-License-Identifier: Apache-2.0
//
// © Copyright Contributors to the Rust Project
// © Copyright 2023,2024 Jan Ole Hüser
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

use base64::Engine as _;

use crate::commandline::*;
use crate::options::*;
use crate::input::*;







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
    let cmdline: &'static [u16] = crate::commandline::get_command_line()?;
    let parsed_args_list : Vec<Arg<'static>> = crate::commandline::parse_lp_cmd_line(cmdline, true);
    let arg0_or_default : std::borrow::Cow<'_, str> = match parsed_args_list.first() {
        Some(arg) => arg.arg.to_string_lossy(),
        None => std::borrow::Cow::from("create-process-rust"),
    };

    let options : options::MainOptions = match options::get_options(cmdline, &parsed_args_list){
        Ok(options) => options,
        Err(msg) => {
            eprintln!("{}\n",msg);
            output::print_args(cmdline, &parsed_args_list,
                       &options::PrintOptions { json: false, silent: false, print_args: true },
                       "", true, &mut std::io::stderr())
                .map_err(|error| error.to_string())?;

            return Err("bad option".to_owned());
        },
    };

    match options.main_choice {
        options::MainChoice::PrintArgs => {
            output::print_args(cmdline, &parsed_args_list, & options.print_opts, "", true, &mut std::io::stdout()).map_err(|error| error.to_string())
        },
        options::MainChoice::Help => {
            options::print_usage(&arg0_or_default, &mut std::io::stdout()).map_err(|x| format!("Print usage failed with: {}", x.to_string()))
        },
        options::MainChoice::ExecOpts(opts) => {
            exec(opts, options.print_opts, cmdline, parsed_args_list)
        },
    }
}


fn print_inner_cmdline(cmdline_opt: &Option<OsString>, print_opts: &options::PrintOptions) -> Result<(), String> {
    match &cmdline_opt {
        Some(cmdline_str)  => {
            let cmdline_vec = cmdline_str.encode_wide().collect_vec();
            let cmdline_slice = &cmdline_vec[..];
            let inner_parsed_args_list = commandline::parse_lp_cmd_line(cmdline_slice, true);
            output::print_args(cmdline_slice, &inner_parsed_args_list, &print_opts, "", false, &mut std::io::stdout())
                .map_err(|error| error.to_string())?;
        },
        _ => {}
    };
    Ok(())
}




fn exec(
    exec_options: options::ExecOptions,
    print_opts: options::PrintOptions,
    cmdline: &'static [u16],
    parsed_args_list : Vec<Arg<'static>>,
)
    -> Result<(), String>
{

    if print_opts.print_args {
        output::print_args(cmdline, &parsed_args_list, &print_opts, "", true, &mut std::io::stdout())
            .map_err(|error| error.to_string())?;
    }

    if exec_options.strip_program
        && (match exec_options.program { options::ProgramOpt::FromCmdLine => false, _ => true })
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

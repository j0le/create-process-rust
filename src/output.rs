use std::{
    ffi::OsString,
    fmt,
    io,
    io::Write,
    os::windows::ffi::OsStrExt,
    os::windows::ffi::OsStringExt,
};

use serde::{
    Serializer,
    ser::SerializeStruct,
};


use crate::commandline::Arg;

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



pub(super) fn print_args<W>(
    cmdline: &[u16],
    parsed_args_list: &Vec<Arg<'_>>,
    print_opts: &crate::options::PrintOptions,
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

use std::fs::File;
use std::{
    ffi::OsStr,
    ffi::OsString,
    io,
};

use serde::{
    Deserialize,
};

#[derive(Deserialize, Debug)]
pub(super) struct JsonUserInput {
    args: Option<Vec<String>>,
    cmdline: Option<String>,
    program: Option<String>,
}

pub(super) enum StdInOrBufReader{
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

pub(super) fn is_filename_stdin(file_name : &OsStr) -> bool {
    file_name == OsStr::new("-")
}

pub(super) fn read_user_input_from_file(file : &OsStr) -> Result<JsonUserInput, String> {
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


pub(super) fn get_cmdline_from_json(json_user_input : &JsonUserInput) -> Result<OsString,String> {
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
pub(super) fn get_program_from_json(json_user_input : &JsonUserInput) -> Result<OsString,String> {

    return Err("TEMP TODO".to_owned());
}

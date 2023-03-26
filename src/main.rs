
use url::Url;
use itertools::Itertools;

fn abs_path(url: &Url) -> Result<std::path::PathBuf, &str> {
    let start_str : &str = "file:///mnt/";
    let opt_url : Option<Url> = if url.as_str().starts_with(start_str) {
        let mut url : String = String::from(url.as_str());
        let (drive_letter, _rest) = match url[start_str.len()..].splitn(2,'/').collect_tuple() {
            Some(it) => it,
            None => return Err("Couldn't convert linux path to windows path"),
        };
        let insert_pos:usize = start_str.len() + drive_letter.len();
        url.replace_range(insert_pos..insert_pos, ":");
        url.replace_range(0..start_str.len(), "file:///");
        Some(Url::parse(&url).unwrap())
    }else{
        None
    };
    let url:&Url = match opt_url.as_ref() {
        Some(u) => &u,
        None => url,
    };
    url.to_file_path().map_err(|()| "not a file")
}

fn main() {

    let moin = b"Jan Ole H\xC3\xBCser";
    if let Ok(my_str) = std::str::from_utf8(moin) {
        println!("And the same as text: '{}'", my_str);
    }

    //println!("hallo {}", moin);

    //let linux_path = "/mnt/c/Users";
    let windows_path = "c:\\heyjo\\was geht";
    let url = match Url::from_file_path(windows_path)
    {
        Ok(url) => {println!("Die Url ist: {}", url.as_str()); url},
        Err(()) => {println!("Got'n error."); return; },
    };

    let driver_letter_range = {
        let (scheme, drive_letter, _rest) = match url.as_str().splitn(3, ':').collect_tuple() {
            Some(it) => it,
            None => return,
        };

        let start : usize = scheme.len() + ':'.len_utf8() ;
        (start + (if drive_letter.starts_with("///") { "///".len() } else {0}))
        ..
        (start + drive_letter.len())
    };

    let mut url: String = url.into();
    url[driver_letter_range.clone()].make_ascii_lowercase();

    // remove colon
    url.replace_range(
        driver_letter_range.end .. (driver_letter_range.end + ':'.len_utf8()),
        ""
        );

    url.replace_range(driver_letter_range.start..driver_letter_range.start, "mnt/");

    println!("The result is {}", url);

    let url : Url = Url::parse(&url).unwrap();
    println!("The url is {}", url);

    match abs_path(&url) {
        Ok(path) => println!("back convert: \"{}\"", path.to_string_lossy()),
        Err(e) => println!("could convert to path: {}",e),
    };
}

// https://stackoverflow.com/questions/43753491/include-git-commit-hash-as-string-into-rust-program/44407625#44407625


use std::process::Command;


fn main() {
    // note: add error checking yourself.
    let output = Command::new("git").args(&["rev-parse", "HEAD"]).env("LC_ALL", "C.UTF-8").output().unwrap();
    if output.status.success() {
        let git_hash = String::from_utf8(output.stdout).expect("git should have output an UTF8 encoded string");
        println!("cargo:rustc-env=GIT_HASH={}", git_hash);
    }
    else{
        println!("cargo:warning=Failed to get git commit id");
    }

    let output = Command::new("git")
        .args(["status", "--untracked-files=no", "--porcelain"])
        .output().expect("git should be executed");
    if output.status.success() {
        println!("cargo:rustc-env=GIT_DIRTY={}",
            if output.stdout.len() == 0 {
                "false"
            }else {
                "true"
            }
        );
    }
    else {
        println!("cargo:warning=Failed to check if git working tree is dirty");
    }
}

// git status --untracked-files=no --porcelain

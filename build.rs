use std::env;
use std::process::Command;

fn main() {
    println!(
        "cargo:rustc-env=TARGET={}",
        std::env::var("TARGET").unwrap()
    );

    match env::var_os("BUILD_REV") {
        None => match get_git_rev() {
            Err(err) => {
                panic!("Failed to get git revision: {err}");
            }
            Ok(rev) => {
                println!("cargo:rustc-env=BUILD_REV={rev}");
            }
        },
        Some(_) => {
            println!(
                "cargo:rustc-env=BUILD_REV={}",
                std::env::var("BUILD_REV").unwrap()
            );
        }
    }
}

fn get_git_rev() -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("--short")
        .arg("HEAD")
        .output()?;
    if !output.status.success() {
        let err = std::string::String::from_utf8_lossy(&output.stderr);
        let err: Box<dyn std::error::Error> = err.into();
        return Err(err);
    }
    let rev = std::string::String::from_utf8_lossy(&output.stdout);
    Ok(rev.into_owned())
}

use std::{env, fs, path::PathBuf};

// Put the right files for the program to work in the build dir
fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();

    let out_dir = env::var("OUT_DIR").map(PathBuf::from).unwrap();
    
    let mut output_dir = out_dir.clone();
    output_dir.pop();
    output_dir.pop();
    output_dir.pop();
    
    let assets = PathBuf::from("assets/");

    fs::create_dir_all(&output_dir).ok();
    
    match (target_os.as_str(), target_arch.as_str()) {
        // Win
        ("windows", "x86_64") => {
            fs::copy(
                assets.join("openxr_loader.dll"),
                output_dir.join("openxr_loader.dll"),
            ).unwrap();
        }
        _ => {
            // Add more as needed
        }
    }
}

use std::option::Option;
use std::path::{Path, PathBuf};

use crate::style::{BOLD, DIM, GREEN};
use anstream::println;

pub(crate) fn detect_project(start: &Path) -> Option<PathBuf> {
    println!("{DIM}cwd: {}{DIM:#}", start.display());
    for dir in start.ancestors() {
        println!("{DIM}{}{DIM:#}", dir.display());
        if dir.join("ProjectSettings/ProjectVersion.txt").is_file() {
            println!(
                "{BOLD}{GREEN}Unity project was found!{GREEN:#}{BOLD:#} {}",
                dir.display()
            );
            return Some(dir.to_path_buf());
        }
    }

    None
}

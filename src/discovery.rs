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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // <root>/ProjectSettings/ProjectVersion.txt を作って root を返す
    fn make_unity_project(root: &Path) {
        let ps = root.join("ProjectSettings");
        fs::create_dir_all(&ps).unwrap();
        fs::write(ps.join("ProjectVersion.txt"), "m_EditorVersion: 6000.0.0f1\n").unwrap();
    }

    #[test]
    fn cwd直下にマーカーがあれば_cwd_を返す() {
        let tmp = TempDir::new().unwrap();
        make_unity_project(tmp.path());

        let found = detect_project(tmp.path());
        assert_eq!(found.as_deref(), Some(tmp.path()));
    }

    #[test]
    fn 親階層にマーカーがあれば_その親を返す() {
        let tmp = TempDir::new().unwrap();
        make_unity_project(tmp.path());
        let deep = tmp.path().join("Assets/Scripts/Foo");
        fs::create_dir_all(&deep).unwrap();

        let found = detect_project(&deep);
        assert_eq!(found.as_deref(), Some(tmp.path()));
    }

    #[test]
    fn rootまでマーカーが無ければ_None() {
        let tmp = TempDir::new().unwrap();
        let deep = tmp.path().join("a/b/c");
        fs::create_dir_all(&deep).unwrap();

        assert_eq!(detect_project(&deep), None);
    }

    #[test]
    fn 存在しないパスでも_panicせず_None() {
        let tmp = TempDir::new().unwrap();
        let gone = tmp.path().join("does/not/exist");

        assert_eq!(detect_project(&gone), None);
    }
}

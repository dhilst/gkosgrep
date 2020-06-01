#![allow(dead_code)]

use log;
use std::env;
use std::error;
use std::fs;
use std::io::{self, BufRead};
use std::path;

#[derive(Debug)]
struct Pattern {
    neg: bool,
    pattern: glob::Pattern,
}

fn to_pattern(pattern: &str) -> Result<Pattern, Box<dyn error::Error>> {
    let mut neg = false;
    let mut recursive = true;
    let mut pattern: String = pattern.to_string();

    if pattern.starts_with("!") {
        neg = true;
        pattern.remove(0);
    }

    if pattern.starts_with("/") {
        pattern.remove(0);
        recursive = false;
    }

    if pattern.ends_with("/") {
        pattern.push_str(if recursive { "**" } else { "*" });
    }

    pattern.insert_str(0, if recursive { "**/" } else { "*/" });

    log::debug!("Compiling {}", pattern);
    let pattern = glob::Pattern::new(&pattern)?;
    Ok(Pattern { neg, pattern })
}

#[derive(Debug)]
struct GitIgnore(Vec<Pattern>);

fn is_pattern(line: &str) -> bool {
    if line.starts_with("#") {
        return false;
    }

    if line.trim().is_empty() {
        return false;
    }

    return true;
}

fn read_gitignore(
    root: &path::Path,
    gitignore_name: &str,
) -> Result<GitIgnore, Box<dyn error::Error>> {
    let file = fs::File::open(path::Path::join(root, gitignore_name))?;
    let file = io::BufReader::new(file);
    let mut patterns = Vec::new();

    for line in file.lines() {
        let line = line?;
        if is_pattern(&line) {
            patterns.push(to_pattern(&line.trim().to_string())?);
        }
    }

    Ok(GitIgnore(patterns))
}

fn matches(pattern: &Pattern, target: &path::Path) -> bool {
    let mut target_ = target.to_str().unwrap().to_string().replace("./", "");
    if target.is_dir() {
        target_.push_str("/");
    }
    let res = pattern.pattern.matches(&target_);

    if res {
        log::debug!(
            "Ignored {:?} {:?} => {:?}",
            pattern.pattern.as_str(),
            target_,
            res
        );
    }

    if pattern.neg {
        !res
    } else {
        res
    }
}

fn ignored(
    target: &path::Path,
    gitignores: &Vec<GitIgnore>,
) -> Result<bool, Box<dyn error::Error>> {
    for gitignore in gitignores.iter().rev() {
        for pattern in &gitignore.0 {
            let matches = matches(&pattern, target);
            if matches {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

fn grep_file(path: &path::Path, pattern: &str) {
    let file = fs::File::open(path);
    if file.is_err() {
        return;
    }
    let file = file.unwrap();
    let file = io::BufReader::new(file);
    for (i, line) in file.lines().enumerate() {
        if line.is_err() {
            return;
        }
        let line = line.unwrap();
        if line.contains(pattern) {
            println!("{}:{}:{}", i, path.to_str().unwrap(), line);
        }
    }
}

pub fn walkdir(root: &path::Path, pattern: &str) -> Result<(), Box<dyn error::Error>> {
    let mut buf: Vec<path::PathBuf> = Vec::new();
    let mut gitignores: Vec<GitIgnore> = Vec::new();
    buf.push(root.into());

    while !buf.is_empty() {
        let dir = buf.remove(0);
        if let Ok(i) = read_gitignore(dir.as_path(), ".gitignore") {
            gitignores.push(i);
        }

        if let Ok(i) = read_gitignore(dir.as_path(), ".ignore") {
            gitignores.push(i);
        }
        for entry in fs::read_dir(dir.as_path())? {
            let entry = entry?;
            let ignored =
                ignored(&entry.path(), &gitignores)? || entry.path().to_str().unwrap() == "./.git";

            if !ignored {
                grep_file(&entry.path(), &pattern);
            }

            if entry.path().is_dir() && !ignored {
                buf.push(entry.path());
            }
        }
        gitignores.pop();
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn error::Error>> {
    let path = env::args().nth(1).unwrap();
    let path = path::Path::new(&path);
    let pattern = env::args().nth(2);
    if pattern.is_none() {
        return Ok(());
    }
    let pattern = pattern.unwrap();
    walkdir(path, &pattern)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_walkdir() {
        env_logger::init();
        walkdir(path::Path::new("."), "let").unwrap();
    }

    #[test]
    fn test_pattern() {
        assert!(glob::Pattern::new("**/target/**")
            .unwrap()
            .matches("target/"));
    }
}

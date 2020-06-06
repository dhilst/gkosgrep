use env_logger;
use log;
use std::env;
use std::error;
use std::fs;
use std::io::{self, BufRead};
use std::path;

fn to_pattern(pattern: &str) -> Result<(bool, glob::Pattern), Box<dyn error::Error>> {
    let original = pattern;
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

    if recursive {
        pattern.insert_str(0, "**/");
    }

    let pattern = glob::Pattern::new(&pattern)?;

    log::debug!("Compiling {} => {}", original, pattern.as_str());

    Ok((neg, pattern))
}

#[derive(Debug)]
struct GitIgnore {
    path: path::PathBuf,
    patterns: Vec<glob::Pattern>,
    neg_patterns: Vec<glob::Pattern>,
}

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
    root: path::PathBuf,
    gitignore_name: &str,
) -> Result<GitIgnore, Box<dyn error::Error>> {
    let file = fs::File::open(path::Path::join(root.as_path(), gitignore_name))?;
    let file = io::BufReader::new(file);
    let mut patterns = Vec::new();
    let mut neg_patterns = Vec::new();

    for line in file.lines() {
        let line = line?;
        if is_pattern(&line) {
            let (neg, pattern) = to_pattern(&line.trim().to_string())?;
            if neg {
                neg_patterns.push(pattern);
            } else {
                patterns.push(pattern);
            }
        }
    }

    log::debug!("{}/{} read", root.to_str().unwrap(), gitignore_name);

    Ok(GitIgnore {
        path: root,
        patterns,
        neg_patterns,
    })
}

fn matches(pattern: &glob::Pattern, target: &path::Path) -> bool {
    let mut target_ = target.to_str().unwrap().to_string().replace("./", "");
    if target.is_dir() {
        target_.push_str("/");
    }
    let res = pattern.matches(&target_);
    log::debug!(
        "Checking {} ~ {} => {}",
        target.to_str().unwrap(),
        pattern.as_str(),
        res
    );
    res
}

fn in_dir(root: &path::Path, target: &path::Path) -> bool {
    target.starts_with(root)
}

fn ignored(
    target: &path::Path,
    gitignores: &Vec<GitIgnore>,
) -> Result<bool, Box<dyn error::Error>> {
    log::debug!("Gitignores count {}", gitignores.iter().count());
    for gitignore in gitignores.iter().rev() {
        for pattern in &gitignore.patterns {
            if !in_dir(gitignore.path.as_path(), target) {
                continue;
            }
            if matches(&pattern, target) {
                log::debug!(
                    "{} ignored by {}",
                    target.to_str().unwrap(),
                    pattern.as_str()
                );
                for neg_pattern in &gitignore.neg_patterns {
                    if matches(neg_pattern, target) {
                        log::debug!(
                            "{} ignored by {}, but reincluded by {}",
                            target.to_str().unwrap(),
                            pattern.as_str(),
                            neg_pattern.as_str()
                        );
                        return Ok(false);
                    }
                }
                return Ok(true);
            }
        }
    }

    Ok(false)
}

fn grep_file(path: &path::Path, pattern: &str) {
    let debug = env::var("RUST_LOG").is_ok();
    if debug {
        log::debug!("{} included", path.to_str().unwrap());
        return;
    }
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
            println!("{}:{}:{}", path.to_str().unwrap(), i + 1, line);
        }
    }
}

pub fn walkdir(root: &path::Path, pattern: &str) -> Result<(), Box<dyn error::Error>> {
    let mut buf: Vec<path::PathBuf> = Vec::new();
    let mut gitignores: Vec<GitIgnore> = Vec::new();
    buf.push(root.into());

    while !buf.is_empty() {
        let dir = buf.remove(0);
        if let Ok(i) = read_gitignore(dir.clone(), ".gitignore") {
            gitignores.push(i);
        }

        if let Ok(i) = read_gitignore(dir.clone(), ".ignore") {
            gitignores.push(i);
        }
        for entry in fs::read_dir(dir.as_path())? {
            let entry = entry?;
            log::debug!("Visiting {}", entry.path().to_str().unwrap());
            let ignored =
                ignored(&entry.path(), &gitignores)? || entry.path().to_str().unwrap() == "./.git";

            if !ignored && entry.path().is_file() {
                grep_file(&entry.path(), &pattern);
            }

            if entry.path().is_dir() && !ignored {
                buf.push(entry.path());
            }
        }
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn error::Error>> {
    env_logger::init();
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

    #[test]
    fn test_pattern2() {
        env_logger::init();
        env::set_current_dir("../mautic").unwrap();
        let gitignore = read_gitignore(path::Path::new(".").to_path_buf(), ".gitignore").unwrap();
        assert!(ignored(path::Path::new("app/cache/dev"), &vec![gitignore]).unwrap());
    }

    #[test]
    fn test_in_dir() {
        env::set_current_dir("../mautic").unwrap();
        assert!(in_dir(
            path::Path::new("."),
            path::Path::new("./app/cache/dev"),
        ));
    }
}

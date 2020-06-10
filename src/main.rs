use env_logger;
use log;
use std::env;
use std::error;
use std::fs;
use std::io::{self, BufRead};
use std::path;
use std::sync;

#[derive(Debug)]
struct Pattern {
    neg: bool,
    pattern: glob::Pattern,
    original: String,
}

static OPTIONS: glob::MatchOptions = glob::MatchOptions {
    case_sensitive: true,
    require_literal_separator: true,
    require_literal_leading_dot: true,
};

impl Pattern {
    fn matches(&self, target: &path::Path) -> bool {
        // plugins/* ~ plugins/
        let res = self.pattern.matches_path_with(target, OPTIONS);
        log::debug!(
            "Checking {} ~ {} => {}",
            target.to_str().unwrap(),
            self.original,
            res
        );
        res
    }

    fn new(pattern: &str) -> Result<Pattern, Box<dyn error::Error>> {
        let original = pattern.to_string();
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

        Ok(Pattern {
            neg,
            original,
            pattern,
        })
    }
}

#[derive(Debug)]
struct GitIgnore {
    path: path::PathBuf,
    patterns: Vec<Pattern>,
    neg_patterns: Vec<Pattern>,
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
            let pattern = Pattern::new(&line.trim().to_string())?;
            if pattern.neg {
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

fn in_dir(root: &path::Path, target: &path::Path) -> io::Result<bool> {
    Ok(target.canonicalize()?.starts_with(root.canonicalize()?))
}

fn ignored(target: &path::Path, gitignores: &Vec<GitIgnore>) -> bool {
    log::debug!("Gitignores count {}", gitignores.iter().count());
    for gitignore in gitignores.iter().rev() {
        if let Ok(false) = in_dir(gitignore.path.as_path(), target) {
            log::debug!(
                "{} not in {}, skipping",
                target.to_str().unwrap(),
                gitignore.path.as_path().to_str().unwrap(),
            );
            continue;
        }
        for pattern in &gitignore.patterns {
            if pattern.matches(target) {
                log::debug!(
                    "{} ignored by {}",
                    target.to_str().unwrap(),
                    pattern.original
                );
                for neg_pattern in &gitignore.neg_patterns {
                    if neg_pattern.matches(target) {
                        log::debug!(
                            "{} ignored by {}, but reincluded by {}",
                            target.to_str().unwrap(),
                            pattern.original,
                            neg_pattern.original,
                        );
                        return false;
                    }
                }
                return true;
            }
        }
    }

    false
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
    let pool = threadpool::ThreadPool::new(num_cpus::get());
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
                ignored(&entry.path(), &gitignores) || entry.path().to_str().unwrap() == "./.git";

            if !ignored && entry.path().is_file() {
                let entry = sync::Arc::new(entry.path());
                let pattern = sync::Arc::new(pattern.to_string());

                pool.execute(move || {
                    grep_file(&entry, &pattern);
                });
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
        env_logger::try_init().ok();
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
        env_logger::try_init().ok();
        env::set_current_dir("../mautic").unwrap();
        let gitignore = read_gitignore(path::Path::new(".").to_path_buf(), ".gitignore").unwrap();
        assert!(ignored(path::Path::new("app/cache/dev"), &vec![gitignore]));
    }

    #[test]
    fn test_pattern3() {
        env_logger::try_init().ok();
        env_logger::try_init().ok();
        env::set_current_dir("../mautic").unwrap();
        let gitignore = read_gitignore(path::Path::new(".").to_path_buf(), ".gitignore").unwrap();
        assert!(!ignored(path::Path::new("./plugins"), &vec![gitignore]));
    }

    #[test]
    fn test_pattern4() {
        assert_eq!(
            false,
            glob::Pattern::new("plugins/*")
                .unwrap()
                .matches_path(path::Path::new("./plugins/"))
        );
    }

    #[test]
    fn test_in_dir() {
        env_logger::try_init().ok();
        env::set_current_dir("../mautic").unwrap();
        assert!(in_dir(path::Path::new("."), path::Path::new("./app/cache/dev"),).unwrap());

        assert!(in_dir(path::Path::new("."), path::Path::new("."),).unwrap());
    }
}

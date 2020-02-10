extern crate regex;
use content_inspector::inspect;
use regex::Regex;
use std::env;
use std::fs::{self, DirEntry, File};
use std::io::{self, prelude::*, BufReader, ErrorKind};
use std::sync::Arc;
use threadpool::ThreadPool;

static THREAD_NUM: usize = 4;

fn main() -> io::Result<()> {
    let pool = ThreadPool::new(THREAD_NUM);
    let path = env::args().nth(1).expect("1th argument not provided");
    let regex = Arc::new(Regex::new(&env::args().nth(2).unwrap_or(String::from(".*"))).unwrap());
    let ignore = Arc::new(GitIgnore::new(vec![".gitignore", ".ignore"]).unwrap());

    walk_dir(path, &ignore, &|entry: DirEntry| {
        let path = entry.path().to_str().unwrap().to_string();
        let ignore = ignore.clone();
        let regex = regex.clone();
        if entry.path().is_file() && !ignore.ignored(&path) {
            pool.execute(move || grep_file(&regex, &path));
        }
    });

    pool.join();

    Ok(())
}

fn walk_dir<F>(path: String, ignores: &GitIgnore, cb: &F) -> ()
where
    F: Fn(DirEntry) -> (),
{
    let specials = ["./.", "./..", "./.git"];
    if !ignores.ignored(&format!("{}/", path)) && !specials.iter().any(|pattern| &path == pattern) {
        match fs::read_dir(&path) {
            Ok(readdir) => {
                for entry in readdir {
                    if entry.is_err() {
                        continue;
                    }
                    let entry = entry.unwrap();
                    let is_dir = entry.path().is_dir();
                    let path = entry.path().to_str().unwrap().to_string();
                    if specials.iter().any(|pattern| &path == pattern) {
                        continue;
                    }

                    if ignores.ignored(&path) {
                        continue;
                    }

                    cb(entry);
                    if is_dir {
                        walk_dir(path, ignores, cb);
                    }
                }
            }
            Err(e) => eprintln!("ERROR: {:?} {} {}", e.kind(), e, path),
        }
    }
}

fn grep_file(regex: &Regex, path: &str) {
    let file = File::open(path).unwrap();
    let reader = BufReader::new(file);
    let mut iter = reader.lines().enumerate();
    let (_, line) = match iter.next() {
        None => return,
        Some(line) => match line {
            (n, Ok(l)) => (n, l),
            (n, Err(e)) => {
                eprintln!("ERROR grep_file {}", e);
                (n, "".into())
            }
        },
    };
    // We only need to check if file is text once,
    // we expect `inspect(line).is_text()` to return
    // true to all lines of the same file
    let line2 = line.clone();
    if !inspect(line2.as_bytes()).is_text() {
        return;
    }
    if regex.is_match(&line) {
        println!("{}:{}:{}", path, 0, line);
    }
    for (i, line) in iter {
        let line = match line {
            Err(err) => {
                match err.kind() {
                    ErrorKind::InvalidData => {}
                    _ => eprintln!("ERROR: {} <{:?}> {}", err, err.kind(), path),
                }
                continue;
            }
            Ok(line) => line,
        };
        if regex.is_match(&line) {
            println!("{}:{}:{}", path, i, line);
        }
    }
}

fn to_glob(ign: &String) -> glob::Pattern {
    let ign = format!(
        "./{}{}",
        ign,
        match ign.chars().last() {
            None => "",
            Some(a) =>
                if a == '/' {
                    "**"
                } else {
                    ""
                },
        }
    );
    glob::Pattern::new(&ign).unwrap()
}

#[derive(Debug)]
struct GitIgnore {
    ignores: Vec<glob::Pattern>,
}

impl GitIgnore {
    pub fn new(paths: Vec<&str>) -> Result<Self, io::Error> {
        let o = GitIgnore {
            ignores: paths
                .iter()
                .map(|x| Self::open(x))
                .flatten()
                .flatten()
                .collect(),
        };
        Ok(o)
    }

    pub fn ignored(&self, path: &String) -> bool {
        self.ignores.iter().any(|ignore| ignore.matches(&path))
    }

    fn open(path: &str) -> Result<Vec<glob::Pattern>, io::Error> {
        match File::open(path) {
            Err(e) => match e.kind() {
                ErrorKind::NotFound => Ok(vec![]),
                _ => Err(e),
            },
            Ok(f) => Ok(BufReader::new(f)
                .lines()
                .map(|x| x.unwrap())
                .map(|x| to_glob(&x))
                .collect()),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_ignored() {
        use super::*;
        let igns = vec!["roles/freeipa/"]
            .iter()
            .map(|x| x.to_string())
            .map(|x| to_glob(&x))
            .collect::<Vec<glob::Pattern>>();
        assert!(igns.iter().any(|file| file.matches("./roles/freeipa/")))
    }
}

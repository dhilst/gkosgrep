extern crate regex;
use content_inspector::inspect;
use regex::Regex;
use std::env;
use std::fs::{self, DirEntry, File};
use std::io::{self, prelude::*, BufReader, ErrorKind};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Duration;
use threadpool::ThreadPool;

static THREAD_NUM: usize = 4;

fn main() -> io::Result<()> {
    let pool = ThreadPool::new(THREAD_NUM);
    let (path_sender, path_receiver) = channel::<String>();
    path_sender
        .send(env::args().nth(1).unwrap().to_string())
        .expect("send");
    let path_sender2 = path_sender.clone();
    drop(path_sender);

    let regex = Arc::new(Regex::new(&env::args().nth(2).unwrap_or(String::from(".*"))).unwrap());
    let git_ignores = Arc::new(GitIgnore::new(".gitignore").unwrap());
    let other_ignores =
        Arc::new(GitIgnore::new(&env::args().nth(3).unwrap_or(".ignore".into())).unwrap());
    let specials = ["./.", "./..", "./.git"];

    loop {
        let pool = pool.clone();
        let path = path_receiver.recv_timeout(Duration::from_millis(200));
        if path.is_err() {
            if pool.queued_count() == 0 {
                break;
            } else {
                continue;
            }
        }
        let path = path.unwrap();
        let path_sender = path_sender2.clone();
        let regex = regex.clone();
        let git_ignores = git_ignores.clone();
        let other_ignores = other_ignores.clone();
        pool.execute(move || {
            visit_dir(&path, |entry| {
                let path: String = entry.path().to_str().unwrap().to_string();
                if entry.path().is_dir() && !specials.iter().any(|s| s == &path) {
                    path_sender.send(path).expect("send");
                } else if entry.path().is_file()
                    && !git_ignores.ignored(&path)
                    && !other_ignores.ignored(&path)
                {
                    grep_file(&regex, entry.path().to_str().unwrap());
                }
            })
        });
    }

    pool.join();

    Ok(())
}

fn visit_dir<F>(path: &str, cb: F) -> ()
where
    F: Fn(DirEntry) -> (),
{
    match fs::read_dir(path) {
        Ok(readdir) => {
            for entry in readdir {
                if entry.is_err() {
                    continue;
                }
                cb(entry.unwrap());
            }
        }
        Err(e) => eprintln!("ERROR: {:?} {} {}", e.kind(), e, path),
    }
}

fn grep_file(regex: &Regex, path: &str) {
    let file = File::open(path).unwrap();
    let reader = BufReader::new(file);
    let mut iter = reader.lines().enumerate();
    let (_, line) = match iter.next() {
        None => return,
        Some(line) => line,
    };
    // We only need to check if file is text once,
    // we expect `inspect(line).is_text()` to return
    // true to all lines of the same file
    if !inspect(line.unwrap_or("".into()).as_bytes()).is_text() {
        return;
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

#[derive(Debug)]
struct GitIgnore {
    ignores: Vec<glob::Pattern>,
}

impl GitIgnore {
    fn new(path: &str) -> Result<GitIgnore, io::Error> {
        let ignores = match File::open(path) {
            Err(e) => {
                match e.kind() {
                    ErrorKind::NotFound => {}
                    _ => eprintln!("ERROR: Error opening {} {} {:?}", path, e, e.kind()),
                };
                Vec::new()
            }
            Ok(f) => BufReader::new(f)
                .lines()
                .map(|pattern| glob::Pattern::new(&format!("./{}", pattern.unwrap())).unwrap())
                .collect::<Vec<glob::Pattern>>(),
        };

        Ok(GitIgnore { ignores })
    }

    fn ignored(&self, path: &str) -> bool {
        self.ignores.iter().any(|ignore| ignore.matches(path))
    }
}

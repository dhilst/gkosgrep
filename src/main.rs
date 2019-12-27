extern crate regex;
use content_inspector::inspect;
use regex::Regex;
use std::env;
use std::fs::{self, DirEntry, File};
use std::io::{self, prelude::*, BufReader, ErrorKind};
use std::sync::mpsc::channel;
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

    let regex = Regex::new(&env::args().nth(2).unwrap_or(String::from(".*"))).unwrap();

    while let Ok(path) = path_receiver.recv_timeout(Duration::from_millis(200)) {
        let path_sender = path_sender2.clone();
        let regex = regex.clone();
        pool.execute(move || {
            visit_dir(&path, |entry| {
                if entry.path().is_dir() {
                    path_sender
                        .send(entry.path().to_str().unwrap().to_string())
                        .expect("send");
                } else if entry.path().is_file() {
                    grep_file(&regex, entry.path().to_str().unwrap());
                }
            })
        });
    }

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
    let (_, line) = iter.next().unwrap();
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

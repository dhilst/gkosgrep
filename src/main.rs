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
    let (tx, rx) = channel::<String>();
    tx.send(env::args().nth(1).unwrap().to_string())
        .expect("send");
    let tx2 = tx.clone();
    drop(tx);

    let pattern = match env::args().nth(2) {
        Some(p) => p,
        None => String::from(".*"),
    };
    let regex = Regex::new(&pattern).unwrap();

    while let Ok(path) = rx.recv_timeout(Duration::from_millis(200)) {
        let tx = tx2.clone();
        let regex = regex.clone();
        pool.execute(move || {
            visit_dir(&path, |entry| {
                if entry.path().is_dir() {
                    tx.send(entry.path().to_str().unwrap().to_string())
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
    for (i, line) in reader.lines().enumerate() {
        if let Err(err) = line {
            match err.kind() {
                ErrorKind::InvalidData => {}
                _ => eprintln!("ERROR: {} <{:?}> {}", err, err.kind(), path),
            }
            continue;
        }
        let line = line.unwrap();
        if inspect(line.as_bytes()).is_text() && regex.is_match(&line) {
            println!("{}:{}:{}", path, i, line);
        }
    }
}

extern crate regex;
use content_inspector::{inspect, ContentType};
use regex::Regex;
use std::env;
use std::fs::{self, DirEntry, File};
use std::io::{self, prelude::*, BufReader, ErrorKind};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use threadpool::ThreadPool;

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

fn main() -> io::Result<()> {
    let (tx, rx) = channel::<String>();
    let tx2 = tx.clone();

    let worker = thread::spawn(move || {
        let pool = ThreadPool::new(4);
        loop {
            let path = match rx.recv_timeout(Duration::from_millis(200)) {
                Err(_) => return,
                Ok(path) => path,
            };
            let tx = tx2.clone();
            pool.execute(move || {
                let pattern: String = match env::args().nth(2) {
                    Some(p) => p,
                    None => String::from(".*"),
                };
                let regex = Regex::new(&pattern).unwrap();
                visit_dir(&path, |entry| {
                    if entry.path().is_dir() {
                        tx.send(entry.path().to_str().unwrap().to_string())
                            .expect("send");
                    } else if entry.path().is_file() {
                        let file = File::open(entry.path()).unwrap();
                        let reader = BufReader::new(file);
                        for (i, line) in reader.lines().enumerate() {
                            if let Err(err) = line {
                                match err.kind() {
                                    ErrorKind::InvalidData => {}
                                    _ => eprintln!(
                                        "ERROR: {} <{:?}> {}",
                                        err,
                                        err.kind(),
                                        entry.path().to_str().unwrap()
                                    ),
                                }
                                continue;
                            }
                            let line = line.unwrap();
                            if inspect(line.as_bytes()).is_text() && regex.is_match(&line) {
                                println!("{}:{}:{}", entry.path().to_str().unwrap(), i, line);
                            }
                        }
                    }
                });
            });
        }
    });

    tx.send(env::args().nth(1).unwrap().to_string())
        .expect("send");

    worker.join().expect("join");

    Ok(())
}

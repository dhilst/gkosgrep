#![feature(test)]

extern crate test;
use content_inspector::inspect;
use glob;
use std::env;
use std::fs::{self, DirEntry, File};
use std::io::{self, prelude::*, BufReader, ErrorKind};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use threadpool::ThreadPool;

fn main() -> io::Result<()> {
    let path = env::args().nth(1).expect("1th argument not provided");
    let pattern = env::args().nth(2).unwrap_or(".*".to_string());
    let ignore = vec![".gitignore", ".ignore"];

    run(&path, &pattern, ignore)
}

fn run(path: &String, pattern: &String, ignores: Vec<&str>) -> io::Result<()> {
    let ignore = Arc::new(GitIgnore::new(ignores).unwrap());
    let pool = ThreadPool::new(8);

    walk_dir(&path, &ignore, &|entry: DirEntry| {
        let path = entry.path().to_str().unwrap().to_string();
        let pattern = pattern.clone();
        if entry.path().is_file() {
            pool.execute(move || grep_file(&pattern, &path));
        }
    });

    pool.join();

    Ok(())
}

fn walk_dir<F>(path: &str, ignores: &GitIgnore, cb: &F) -> ()
where
    F: Fn(DirEntry) -> (),
{
    let specials = [".git/"];
    if !ignores.ignored(&format!("{}/", path))
        && !specials.iter().any(|pattern| path.contains(pattern))
    {
        match fs::read_dir(&path) {
            Ok(readdir) => {
                for entry in readdir {
                    if entry.is_err() {
                        continue;
                    }
                    let entry = entry.unwrap();
                    let is_dir = entry.path().is_dir();
                    let path = entry.path().to_str().unwrap().to_string();
                    if specials.iter().any(|pattern| path.contains(pattern)) {
                        continue;
                    }

                    if ignores.ignored(&path) {
                        continue;
                    }

                    cb(entry);

                    if is_dir {
                        walk_dir(&path, ignores, cb);
                    }
                }
            }
            Err(e) => eprintln!("ERROR: {:?} {} {}", e.kind(), e, path),
        }
    } else {
    }
}

fn println(path: &str, linum: usize, line: &str) {
    println!("{}:{}:{}", path, linum, line);
}

fn grep_file(pattern: &String, path: &str) {
    let file = File::open(path).unwrap();
    let reader = BufReader::with_capacity(8 * 1024, file);
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
    if line.contains(pattern) {
        println(path, 0, &line);
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
        if line.contains(pattern) {
            println(path, i, &line);
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
        self.ignores
            .iter()
            .any(|ignore| if ignore.matches(&path) { true } else { false })
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
    use super::*;
    use std::path::Path;
    use test::Bencher;
    #[test]
    fn test_ignored() {
        use super::*;
        let igns = vec!["roles/freeipa/"]
            .iter()
            .map(|x| x.to_string())
            .map(|x| to_glob(&x))
            .collect::<Vec<glob::Pattern>>();
        assert!(igns.iter().any(|file| file.matches("./roles/freeipa/")));

        let igns = vec!["./target/**"]
            .iter()
            .map(|x| x.to_string())
            .map(|x| to_glob(&x))
            .collect::<Vec<glob::Pattern>>();
        assert!(!igns
            .iter()
            .any(|file| file.matches("../linux/arch/arc/kernel/stacktrace.c")));
    }

    #[test]
    fn test_glob() {
        assert_eq!(
            false,
            glob::Pattern::new("./target/**")
                .unwrap()
                .matches("../target/foo/bar")
        );
    }

    #[bench]
    fn bench_ignores(b: &mut Bencher) {
        let igns = vec!["roles/freeipa/"]
            .iter()
            .map(|x| x.to_string())
            .map(|x| to_glob(&x))
            .collect::<Vec<glob::Pattern>>();
        b.iter(|| igns.iter().any(|file| file.matches("./roles/freeipa/")))
    }

    #[bench]
    fn bench_grep(b: &mut Bencher) {
        b.iter(|| {
            grep_file(
                &"and".to_string(),
                "/home/dhilst/Downloads/cantrbry/alice29.txt",
            )
        })
    }

    #[bench]
    fn bench_alice(b: &mut Bencher) {
        let gign = vec![] as Vec<&str>;
        let pat = "and".to_string();
        b.iter(|| run(&"./test".to_string(), &pat, gign.to_vec()));
    }

    #[bench]
    fn bench_walkdir(b: &mut Bencher) {
        b.iter(|| walk_dir("./src/".into(), &GitIgnore::new(vec![]).unwrap(), &|_| ()));
    }

    #[bench]
    fn bench_kernel(b: &mut Bencher) {
        let ksrc = env::var("KERNEL_SRC")
            .expect("KERNEL_SRC not set up")
            .to_string();
        let src = Path::new(&ksrc);
        let src = Arc::new(src.join(".gitignore").to_str().unwrap().to_string());
        let gign = Arc::new(vec![src.as_str()]);
        let pat = "struct".to_string();

        b.iter(|| {
            run(
                &Path::new(&ksrc)
                    .join("arch/arm")
                    .to_str()
                    .unwrap()
                    .to_string(),
                &pat,
                gign.to_vec(),
            )
        })
    }
}

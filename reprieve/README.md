# reprieve

A dead-simple executor for use with the new async/await.

This library aims to have minimal dependencies and compile quickly.
It is intended for projects that can benefit from the mental model of futures but don't need a high-performance network runtime;
I wrote it to make work on a streaming compiler easier.
Provides a function `spawn` to spawn a future on a threadpool,
and `unblock` to spawn a unit of blocking work on a blocking threadpool,
and `wait` to wait for any future.

### Example with std::io

```rust
use std::{
    fs::{self, DirEntry},
    io,
    path::Path,
};

async fn read_to_string<P: AsRef<Path>>(path: P) -> io::Result<String> {
    let path = path.as_ref().to_owned();
    reprieve::unblock(move || fs::read_to_string(path)).await
}

async fn read_dir<P: AsRef<Path>>(path: P) -> io::Result<Vec<DirEntry>> {
    let path = path.as_ref().to_owned();
    reprieve::unblock(move || {
        let mut results = vec![];
        for entry in fs::read_dir(path)? {
            results.push(entry?);
        }
        Ok(results)
    })
    .await
}

async fn read_all_files<P: AsRef<Path>>(path: P) -> io::Result<Vec<(String, String)>> {
    let mut lookups = vec![];
    for entry in read_dir(path).await? {
        if entry.path().is_dir() { continue }
        lookups.push((
            entry.file_name().to_string_lossy().into(),
            reprieve::spawn(read_to_string(entry.path())),
        ));
    }
    // collect:
    let mut results = vec![];
    for (name, data) in lookups {
        results.push((name, data.await?));
    }
    Ok(results)
}

fn main() -> io::Result<()> {
    for (name, data) in reprieve::wait(read_all_files("."))? {
        println!("{}: {}", name, data.lines().next().unwrap());
    }
    Ok(())
}
```

# reprieve

Convert blocking code into futures.

### Example with std::io

```rust
use std::{
    io, fs, path::Path,
};

// declare the error type you want to use in this module
// alternatively, `use reprieve::unblock;`
reprieve::use_error!(io::Error);

async fn read_to_string<P: AsRef<Path>>(path: P) -> io::Result<String> {
    // convert blocking code to a future
    let result = reprieve::unblocked! {
        let path = path.as_ref().to_owned();
        fs::read_to_string(path)?
    };
    result.await
}
```

use super::{Error, Result};
use log::{error, info, warn};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

macro_rules! try_cont {
    ($v:expr) => {
        match $v {
            Ok(v) => v,
            _ => continue,
        }
    };
    (@ $v:expr) => {
        match $v {
            Some(v) => v,
            _ => continue,
        }
    };
}

/// Ensure that rls analysis data is available and up to date.
pub fn ensure_analysis(path: &Path) -> Result<()> {
    let status = Command::new("cargo")
        .args(&["check"])
        .current_dir(path)
        .status()?;

    if !status.success() {
        return Err(Error::CargoCheckFailed);
    }

    let mut rls = Command::new("rls")
        .args(&["--cli"])
        .current_dir(path)
        .stdout(Stdio::piped())
        .spawn()?;

    let mut stdout = None;
    std::mem::swap(&mut rls.stdout, &mut stdout);

    let reader = BufReader::new(stdout.ok_or(Error::Other {
        cause: "can't get rls stdout",
    })?);

    let (tx, rx) = std::sync::mpsc::channel::<Progress>();

    let read_thread = std::thread::spawn(move || {
        let mut started = false;
        for line in reader.lines() {
            let line = match line {
                Ok(line) => line,
                Err(e) => {
                    error!("failed to check rls output: {}", e);
                    tx.send(Progress::Failure).unwrap();
                    return;
                }
            };
            if !started {
                started = true;
                tx.send(Progress::Started).unwrap();
            }
            let value = try_cont!(serde_json::from_str::<Value>(&line));

            if value.get("jsonrpc") != Some(&json!("2.0")) {
                warn!("unexpected RLS version: {:?}", value.get("jsonrpc"));
            }
            if value.get("method") != Some(&json!("window/progress")) {
                continue;
            }

            let params = try_cont!(@value.get("params"));
            let params = try_cont!(@params.as_object());
            if params.get("title") == Some(&json!("Building")) {
                if params.get("done") == Some(&json!(true)) {
                    tx.send(Progress::Done).unwrap();
                    return;
                }
                let message = try_cont!(@params.get("message"));
                let message = try_cont!(@message.as_str());
                info!("rls building: {}", message);
            }
        }
    });
    std::mem::forget(read_thread);

    // manager thread
    let started = rx.recv_timeout(Duration::from_secs(20));
    if started != Ok(Progress::Started) {
        rls.kill()?;
        return Err(Error::RlsFailed);
    }
    let finished = rx.recv();
    if finished != Ok(Progress::Done) {
        rls.kill()?;
        return Err(Error::RlsFailed);
    }
    rls.kill()?;

    Ok(())
}

#[derive(PartialEq, Eq)]
enum Progress {
    Started,
    Done,
    Failure,
}

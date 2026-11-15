//! Background jobs whose command output streams into the UI line by line,
//! the same way App Hub shows its live log (e.g. `Downloading … (1/1)`).

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};

pub enum JobEvent {
    /// One line of stdout/stderr from the child process.
    Line(String),
    /// The process exited. `code` is the exit status (non-zero on failure)
    /// and `log` is the complete captured output.
    Finished { code: i32, log: String },
}

/// Spawn `cmd` with piped stdio and forward every output line through the
/// returned receiver. The complete log is sent with `Finished`.
///
/// stdout and stderr are both captured (merged, like App Hub's helper).
pub fn spawn(mut cmd: Command) -> Receiver<JobEvent> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(error) => {
                let _ = tx.send(JobEvent::Finished {
                    code: -1,
                    log: format!("Could not start the command: {error}"),
                });
                return;
            }
        };
        let out_tx = tx.clone();
        let err_tx = tx.clone();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let out_thread = std::thread::spawn(move || stream_lines(stdout, out_tx));
        let err_thread = std::thread::spawn(move || stream_lines(stderr, err_tx));
        let out_log = out_thread.join().unwrap_or_default();
        let err_log = err_thread.join().unwrap_or_default();
        let status = child.wait();
        let code = status.map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);
        let mut log = String::new();
        if !out_log.is_empty() {
            log.push_str(&out_log);
        }
        if !err_log.is_empty() {
            if !log.is_empty() {
                log.push('\n');
            }
            log.push_str(&err_log);
        }
        let _ = tx.send(JobEvent::Finished { code, log });
    });
    rx
}

fn stream_lines<R: std::io::Read + Send + 'static>(
    pipe: Option<R>,
    tx: mpsc::Sender<JobEvent>,
) -> String {
    let mut buf = String::new();
    let Some(pipe) = pipe else {
        return buf;
    };
    let reader = BufReader::new(pipe);
    for line in reader.lines() {
        match line {
            Ok(line) => {
                let _ = tx.send(JobEvent::Line(line.clone()));
                buf.push_str(&line);
                buf.push('\n');
            }
            Err(_) => break,
        }
    }
    buf
}

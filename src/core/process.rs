use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};

use crate::log_bus::LogEvent;

#[cfg(windows)]
use crate::core::win::CREATE_NO_WINDOW;

/// Maximum number of stderr lines retained for the "last exit" error
/// banner. Big enough to capture sing-box's standard config-error
/// stack, small enough not to bloat memory.
const STDERR_TAIL_CAP: usize = 20;

pub struct ProcessHandle {
    child: Mutex<Option<Child>>,
    /// Recent stderr lines, captured by the stderr pump thread. Drained
    /// by the exit watcher to compose a useful error message.
    stderr_tail: Arc<Mutex<VecDeque<String>>>,
    /// Set true by `stop()` so the exit watcher can distinguish a
    /// user-initiated kill (silent) from an unexpected sing-box crash
    /// (surfaced as `last_exit_error`).
    expected_exit: Arc<AtomicBool>,
    /// One-shot error string, populated by the watcher when sing-box
    /// exits unexpectedly. Drained by the UI on each frame and shown
    /// in the red banner of the status card.
    last_exit_error: Mutex<Option<String>>,
}

impl ProcessHandle {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            child: Mutex::new(None),
            stderr_tail: Arc::new(Mutex::new(VecDeque::with_capacity(STDERR_TAIL_CAP))),
            expected_exit: Arc::new(AtomicBool::new(false)),
            last_exit_error: Mutex::new(None),
        })
    }

    /// Returns true if the child is still alive. The watcher thread
    /// reaps the slot on its own, so this is just a presence check —
    /// no `try_wait` race with the watcher.
    pub fn is_running(&self) -> bool {
        self.child.lock().unwrap().is_some()
    }

    /// Take whatever error string the watcher last recorded, if any.
    /// Called by the UI thread once per frame.
    pub fn take_exit_error(&self) -> Option<String> {
        self.last_exit_error.lock().unwrap().take()
    }

    pub fn start(
        self: &Arc<Self>,
        core_exe: &Path,
        config_file: &Path,
        working_dir: &Path,
        log_tx: Sender<LogEvent>,
    ) -> Result<()> {
        if self.is_running() {
            return Err(anyhow!("sing-box is already running"));
        }
        if !core_exe.exists() {
            return Err(anyhow!("core not found: {}", core_exe.display()));
        }
        if !config_file.exists() {
            return Err(anyhow!("config not found: {}", config_file.display()));
        }

        let mut cmd = Command::new(core_exe);
        cmd.arg("run")
            .arg("-c")
            .arg(config_file)
            .arg("-D")
            .arg(working_dir)
            .current_dir(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd
            .spawn()
            .with_context(|| format!("failed to spawn: {}", core_exe.display()))?;

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        // A fresh start clears any prior crash record and the tail so
        // we don't conflate this run's diagnostics with the previous one.
        self.stderr_tail.lock().unwrap().clear();
        *self.last_exit_error.lock().unwrap() = None;
        self.expected_exit.store(false, Ordering::SeqCst);
        *self.child.lock().unwrap() = Some(child);

        if let Some(out) = stdout {
            let tx = log_tx.clone();
            thread::spawn(move || pump_lines(out, tx, None));
        }
        if let Some(err) = stderr {
            let tx = log_tx.clone();
            let tail = self.stderr_tail.clone();
            thread::spawn(move || pump_lines(err, tx, Some(tail)));
        }

        // Exit-watcher thread. Polls try_wait so it never races with a
        // user-initiated `stop()` that needs its own wait() — there is
        // only one consumer of the Child slot at a time.
        {
            let this = self.clone();
            let log_tx = log_tx.clone();
            thread::spawn(move || this.watch_for_exit(log_tx));
        }

        let _ = log_tx.send(LogEvent::line(format!(
            "[launcher] started {} (config: {})",
            core_exe.file_name().and_then(|s| s.to_str()).unwrap_or(""),
            config_file
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(""),
        )));
        Ok(())
    }

    /// Background watcher: polls the child for exit. On unexpected
    /// (non user-initiated) termination, builds an error message from
    /// the captured stderr tail and stashes it in `last_exit_error`
    /// for the UI to surface in the red banner.
    fn watch_for_exit(self: Arc<Self>, log_tx: Sender<LogEvent>) {
        loop {
            thread::sleep(Duration::from_millis(250));
            let mut guard = self.child.lock().unwrap();
            let Some(child) = guard.as_mut() else {
                // Slot already cleared (probably by `stop()`); nothing
                // left for us to do — the user-initiated path handles
                // the wait itself.
                return;
            };
            match child.try_wait() {
                Ok(Some(status)) => {
                    *guard = None;
                    drop(guard);

                    // A `stop()` request is the *only* "expected" exit
                    // — anything else (success or crash) gets reported
                    // in the log; non-zero also raises the banner.
                    let was_expected = self.expected_exit.swap(false, Ordering::SeqCst);
                    if was_expected {
                        return;
                    }

                    let code_str = match status.code() {
                        Some(c) => format!("exit code {c}"),
                        None => "terminated by signal".to_string(),
                    };
                    let _ = log_tx.send(LogEvent::line(format!(
                        "[launcher] sing-box exited ({code_str})"
                    )));

                    if !status.success() {
                        let tail = {
                            let q = self.stderr_tail.lock().unwrap();
                            q.iter().cloned().collect::<Vec<_>>()
                        };
                        let msg = format_exit_error(&code_str, &tail);
                        *self.last_exit_error.lock().unwrap() = Some(msg);
                    }
                    return;
                }
                Ok(None) => {
                    // Still running — keep polling.
                    drop(guard);
                }
                Err(_) => {
                    // try_wait failure is rare; surface as "exited"
                    // so we don't loop forever on a half-dead handle.
                    *guard = None;
                    return;
                }
            }
        }
    }

    pub fn stop(&self, log_tx: Sender<LogEvent>) -> Result<()> {
        let mut guard = self.child.lock().unwrap();
        if let Some(mut child) = guard.take() {
            // Mark this exit as expected BEFORE killing so any race
            // with the watcher resolves to "no error reported".
            self.expected_exit.store(true, Ordering::SeqCst);
            let _ = child.kill();
            let _ = child.wait();
            let _ = log_tx.send(LogEvent::line("[launcher] sing-box stopped"));
        }
        // User-initiated stop never produces a banner error, even if
        // sing-box happened to crash a few ms before our kill.
        *self.last_exit_error.lock().unwrap() = None;
        Ok(())
    }
}

/// Compose a one-line summary plus the most relevant stderr context.
/// We grep the tail for the line(s) sing-box typically uses to report
/// a fatal error (`FATAL`, `panic`, `error`); falling back to the last
/// non-empty line if none of those markers are present.
fn format_exit_error(code_str: &str, tail: &[String]) -> String {
    let pick = tail
        .iter()
        .rev()
        .find(|l| {
            let lc = l.to_ascii_lowercase();
            lc.contains("fatal") || lc.contains("panic") || lc.contains("error")
        })
        .or_else(|| tail.iter().rev().find(|l| !l.trim().is_empty()))
        .cloned();
    match pick {
        Some(line) => format!("sing-box exited ({code_str}): {}", line.trim()),
        None => format!("sing-box exited ({code_str}). See log for details."),
    }
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        // Best-effort cleanup so we never leak a running sing-box when the
        // launcher window is closed.
        if let Ok(mut guard) = self.child.lock() {
            if let Some(mut child) = guard.take() {
                self.expected_exit.store(true, Ordering::SeqCst);
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

/// Read lines from `reader`, forward each as a `LogEvent`, and (when
/// `tail` is provided) push the line into a bounded ring buffer used
/// by the exit watcher to compose the crash-banner message.
fn pump_lines<R: std::io::Read>(
    reader: R,
    tx: Sender<LogEvent>,
    tail: Option<Arc<Mutex<VecDeque<String>>>>,
) {
    let buf = BufReader::new(reader);
    for line in buf.lines().map_while(Result::ok) {
        // sing-box emits ANSI color codes (\x1b[31m, \x1b[0m, …) on its
        // stderr; the egui default font has no glyph for the ESC byte
        // (U+001B) and renders each escape sequence as replacement
        // boxes. Strip them once here so both the live log and the
        // crash-banner picker see clean text.
        let line = strip_ansi(&line);
        if let Some(t) = &tail {
            let mut q = t.lock().unwrap();
            if q.len() == STDERR_TAIL_CAP {
                q.pop_front();
            }
            q.push_back(line.clone());
        }
        if tx.send(LogEvent::line(line)).is_err() {
            break;
        }
    }
}

/// Remove ANSI CSI escape sequences (`ESC [ … letter`) and any other
/// stray ESC bytes. Lightweight hand-rolled scanner so we don't pull
/// in the `regex` crate just for log cleanup.
fn strip_ansi(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == 0x1B {
            // ESC. If followed by '[' it's a CSI sequence: skip until
            // the final byte in 0x40–0x7E. Otherwise drop just the ESC.
            if i + 1 < bytes.len() && bytes[i + 1] == b'[' {
                i += 2;
                while i < bytes.len() && !(0x40..=0x7E).contains(&bytes[i]) {
                    i += 1;
                }
                if i < bytes.len() {
                    i += 1; // consume final byte
                }
            } else {
                i += 1;
            }
            continue;
        }
        // Push the next UTF-8 char in one shot to keep multi-byte
        // sequences intact.
        let ch_start = i;
        let width = utf8_char_width(b);
        let end = (ch_start + width).min(bytes.len());
        if let Ok(chunk) = std::str::from_utf8(&bytes[ch_start..end]) {
            out.push_str(chunk);
        }
        i = end.max(ch_start + 1);
    }
    out
}

/// UTF-8 leading-byte → total byte length of the encoded scalar.
fn utf8_char_width(b: u8) -> usize {
    if b < 0xC0 {
        1 // continuation byte, treat as single (defensive)
    } else if b < 0xE0 {
        2
    } else if b < 0xF0 {
        3
    } else {
        4
    }
}

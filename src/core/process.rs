use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{anyhow, Context, Result};

use crate::log_bus::LogEvent;

/// CREATE_NO_WINDOW — prevents a black console window from flashing when
/// spawning the sing-box child process under a GUI subsystem.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct ProcessHandle {
    child: Mutex<Option<Child>>,
}

impl ProcessHandle {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            child: Mutex::new(None),
        })
    }

    /// Returns true if the child is still alive. Reaps zombie state so the
    /// UI flips back to "stopped" automatically when sing-box exits on its own.
    pub fn is_running(&self) -> bool {
        let mut guard = self.child.lock().unwrap();
        match guard.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(Some(_)) => {
                    *guard = None;
                    false
                }
                Ok(None) => true,
                Err(_) => true,
            },
            None => false,
        }
    }

    pub fn start(
        &self,
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
        *self.child.lock().unwrap() = Some(child);

        if let Some(out) = stdout {
            let tx = log_tx.clone();
            thread::spawn(move || pump_lines(out, tx));
        }
        if let Some(err) = stderr {
            let tx = log_tx.clone();
            thread::spawn(move || pump_lines(err, tx));
        }

        let _ = log_tx.send(LogEvent::line(format!(
            "[launcher] started {} (config: {})",
            core_exe.file_name().and_then(|s| s.to_str()).unwrap_or(""),
            config_file.file_name().and_then(|s| s.to_str()).unwrap_or(""),
        )));
        Ok(())
    }

    pub fn stop(&self, log_tx: Sender<LogEvent>) -> Result<()> {
        let mut guard = self.child.lock().unwrap();
        if let Some(mut child) = guard.take() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = log_tx.send(LogEvent::line("[launcher] sing-box stopped"));
        }
        Ok(())
    }
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        // Best-effort cleanup so we never leak a running sing-box when the
        // launcher window is closed.
        if let Ok(mut guard) = self.child.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

fn pump_lines<R: std::io::Read>(reader: R, tx: Sender<LogEvent>) {
    let buf = BufReader::new(reader);
    for line in buf.lines().map_while(Result::ok) {
        if tx.send(LogEvent::line(line)).is_err() {
            break;
        }
    }
}

//! PTY spawn + background reader thread (bytes → channel).

use std::io::{Read, Write};
use std::path::Path;
use std::thread::JoinHandle;

use crossbeam_channel::Receiver;
use portable_pty::{native_pty_system, Child, CommandBuilder, ExitStatus, MasterPty, PtySize};

use crate::error::TerminalError;
use crate::shell::ShellConfig;

/// Bytes from the PTY slave, or EOF when the reader exits.
#[derive(Debug)]
pub enum PtyReadMsg {
    Data(Vec<u8>),
    Eof,
}

/// Owns master side + child; [`Terminal`](crate::terminal::Terminal) drains [`PtyReadMsg`] each poll.
pub struct TerminalProcess {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    reader_handle: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for TerminalProcess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerminalProcess").finish_non_exhaustive()
    }
}

impl TerminalProcess {
    /// Spawns `shell` in `cwd` with `size`, returns PTY output channel.
    pub fn spawn(
        shell: &ShellConfig,
        cwd: impl AsRef<Path>,
        size: PtySize,
    ) -> Result<(Self, Receiver<PtyReadMsg>), TerminalError> {
        let cwd = cwd.as_ref();
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(size)?;
        let mut cmd = CommandBuilder::new(&shell.program);
        for a in &shell.args {
            cmd.arg(a);
        }
        cmd.cwd(cwd);
        for (k, v) in &shell.env {
            cmd.env(k, v);
        }
        let child = pair.slave.spawn_command(cmd).map_err(|e| TerminalError::Pty(e.to_string()))?;
        let mut reader =
            pair.master.try_clone_reader().map_err(|e| TerminalError::Pty(e.to_string()))?;
        let writer = pair.master.take_writer().map_err(|e| TerminalError::Pty(e.to_string()))?;
        let master = pair.master;

        let (tx, rx) = crossbeam_channel::unbounded();
        let handle = std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        let _ = tx.send(PtyReadMsg::Eof);
                        break;
                    }
                    Ok(n) => {
                        let _ = tx.send(PtyReadMsg::Data(buf[..n].to_vec()));
                    }
                    Err(_) => {
                        let _ = tx.send(PtyReadMsg::Eof);
                        break;
                    }
                }
            }
        });

        Ok((Self { master, writer, child, reader_handle: Some(handle) }, rx))
    }

    pub fn write(&mut self, bytes: &[u8]) -> Result<(), TerminalError> {
        self.writer.write_all(bytes)?;
        self.writer.flush()?;
        Ok(())
    }

    pub fn resize(&mut self, size: PtySize) -> Result<(), TerminalError> {
        self.master.resize(size).map_err(|e| TerminalError::Pty(e.to_string()))
    }

    pub fn try_wait(&mut self) -> Result<Option<ExitStatus>, TerminalError> {
        Ok(self.child.try_wait()?)
    }

    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(h) = self.reader_handle.take() {
            let _ = h.join();
        }
    }
}

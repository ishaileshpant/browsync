use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// IPC message types between CLI and daemon
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcMessage {
    /// Request daemon status
    #[serde(rename = "status")]
    Status,

    /// Trigger an immediate sync
    #[serde(rename = "sync")]
    Sync {
        browser: Option<String>,
    },

    /// Request the daemon to stop
    #[serde(rename = "stop")]
    Stop,

    /// Status response
    #[serde(rename = "status_response")]
    StatusResponse {
        running: bool,
        watching: Vec<String>,
        last_sync: Option<String>,
        syncs_total: u64,
    },

    /// Ack response
    #[serde(rename = "ack")]
    Ack {
        message: String,
    },
}

/// Path to the Unix domain socket
pub fn socket_path() -> PathBuf {
    let dir = dirs::home_dir()
        .expect("home dir")
        .join(".browsync");
    dir.join("browsync.sock")
}

/// IPC Server (used by daemon)
pub struct IpcServer {
    listener: UnixListener,
}

impl IpcServer {
    pub fn bind() -> Result<Self> {
        let path = socket_path();
        // Remove stale socket
        let _ = std::fs::remove_file(&path);

        let listener = UnixListener::bind(&path)
            .with_context(|| format!("Binding IPC socket at {}", path.display()))?;

        // Set non-blocking for the listener so accept doesn't block forever
        listener.set_nonblocking(true)?;

        eprintln!("IPC listening on {}", path.display());
        Ok(Self { listener })
    }

    /// Try to accept a connection and read a message (non-blocking)
    pub fn try_recv(&self) -> Option<(IpcMessage, UnixStream)> {
        match self.listener.accept() {
            Ok((stream, _)) => {
                let _ = stream.set_nonblocking(false);
                let mut reader = BufReader::new(&stream);
                let mut line = String::new();
                if let Ok(n) = reader.read_line(&mut line) {
                    if n > 0 {
                        if let Ok(msg) = serde_json::from_str::<IpcMessage>(&line) {
                            return Some((msg, stream));
                        }
                    }
                }
                None
            }
            Err(_) => None,
        }
    }

    /// Send a response to a client
    pub fn respond(mut stream: UnixStream, msg: &IpcMessage) -> Result<()> {
        let json = serde_json::to_string(msg)?;
        stream.write_all(json.as_bytes())?;
        stream.write_all(b"\n")?;
        stream.flush()?;
        Ok(())
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(socket_path());
    }
}

/// IPC Client (used by CLI)
pub struct IpcClient;

impl IpcClient {
    /// Send a message to the daemon and get a response
    pub fn send(msg: &IpcMessage) -> Result<IpcMessage> {
        let path = socket_path();
        let mut stream = UnixStream::connect(&path)
            .with_context(|| "Daemon not running. Start with `browsync daemon start`")?;

        let json = serde_json::to_string(msg)?;
        stream.write_all(json.as_bytes())?;
        stream.write_all(b"\n")?;
        stream.flush()?;

        let mut reader = BufReader::new(&stream);
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let response = serde_json::from_str(&line)?;
        Ok(response)
    }

    /// Check if daemon is running
    pub fn is_running() -> bool {
        socket_path().exists() && Self::send(&IpcMessage::Status).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipc_message_serialization() {
        let msg = IpcMessage::StatusResponse {
            running: true,
            watching: vec!["Chrome".to_string()],
            last_sync: Some("2024-01-01".to_string()),
            syncs_total: 42,
        };

        let json = serde_json::to_string(&msg).unwrap();
        let parsed: IpcMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            IpcMessage::StatusResponse {
                running,
                watching,
                syncs_total,
                ..
            } => {
                assert!(running);
                assert_eq!(watching.len(), 1);
                assert_eq!(syncs_total, 42);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_socket_path() {
        let path = socket_path();
        assert!(path.to_str().unwrap().contains(".browsync"));
        assert!(path.to_str().unwrap().ends_with("browsync.sock"));
    }
}

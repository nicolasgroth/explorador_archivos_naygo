// Naygo — fixture Windows de lectura bloqueada, sin tocar archivos ni sesión personal.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use std::os::windows::process::CommandExt;
use std::{
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};

pub struct BlockedPipe {
    child: Child,
    messages: mpsc::Receiver<String>,
    name: String,
}
impl BlockedPipe {
    pub fn new() -> Self {
        // Windows PowerShell/.NET is used only by this test fixture. The product adds
        // no dependency. The hidden server owns exactly one ephemeral named pipe.
        let name = format!(
            "naygo-preview-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let script = format!("$p=[System.IO.Pipes.NamedPipeServerStream]::new('{name}',[System.IO.Pipes.PipeDirection]::Out); try {{ [Console]::WriteLine('ready'); $p.WaitForConnection(); [Console]::WriteLine('connected'); [Console]::In.ReadLine() | Out-Null }} finally {{ $p.Dispose() }}");
        let mut child = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .creation_flags(0x08000000)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let output = child.stdout.take().unwrap();
        let (tx, messages) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(output).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let pipe = Self {
            child,
            messages,
            name,
        };
        assert_eq!(
            pipe.messages.recv_timeout(Duration::from_secs(15)).unwrap(),
            "ready"
        );
        pipe
    }
    pub fn path(&self) -> PathBuf {
        format!(r"\\.\pipe\{}", self.name).into()
    }
    pub fn wait_connected(&self) {
        assert_eq!(
            self.messages.recv_timeout(Duration::from_secs(10)).unwrap(),
            "connected"
        );
    }
}
impl Drop for BlockedPipe {
    fn drop(&mut self) {
        // Only this owned test helper. Closing its pipe also unblocks any failed test reader.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

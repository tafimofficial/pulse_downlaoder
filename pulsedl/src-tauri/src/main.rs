// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let is_minimized = args.iter().any(|a| a == "--minimized" || a == "--tray");

    // Check if an instance of PulseDL is already running on port 45732
    if let Ok(mut stream) = TcpStream::connect_timeout(
        &"127.0.0.1:45732".parse().unwrap(),
        Duration::from_millis(300),
    ) {
        if !is_minimized {
            // Signal running instance to restore and show its main window
            let req = "GET /api/show HTTP/1.1\r\nHost: 127.0.0.1:45732\r\nConnection: close\r\n\r\n";
            let _ = stream.write_all(req.as_bytes());
            let mut buf = [0u8; 128];
            let _ = stream.read(&mut buf);
        }
        // Exit this second launcher process cleanly
        return;
    }

    pulsedl_lib::run();
}

use crate::commands;
use crate::resp::{self, Value};
use crate::store::Store;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn serve(addr: &str, store: Arc<Mutex<Store>>) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr)?;
    println!("rudis listening on {addr} — point redis-cli at it");
    for conn in listener.incoming() {
        let Ok(conn) = conn else { continue };
        let store = store.clone();
        std::thread::spawn(move || {
            let _ = handle(conn, store);
        });
    }
    Ok(())
}

fn handle(mut conn: TcpStream, store: Arc<Mutex<Store>>) -> std::io::Result<()> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        // drain every complete command currently in the buffer
        loop {
            match resp::parse_command(&buf) {
                Ok(Some((args, used))) => {
                    buf.drain(..used);
                    if args.is_empty() {
                        continue;
                    }
                    let quit = args[0].eq_ignore_ascii_case(b"QUIT");
                    let reply = {
                        let mut s = store.lock().unwrap();
                        commands::dispatch(&mut s, &args, now_ms())
                    };
                    let mut out = Vec::new();
                    resp::encode(&reply, &mut out);
                    conn.write_all(&out)?;
                    if quit {
                        return Ok(());
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    let mut out = Vec::new();
                    resp::encode(&Value::Error(format!("ERR {e}")), &mut out);
                    conn.write_all(&out)?;
                    buf.clear();
                    break;
                }
            }
        }
        let n = conn.read(&mut chunk)?;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

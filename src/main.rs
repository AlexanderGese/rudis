mod commands;
mod resp;
mod store;

use std::io::{BufRead, Write};

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn main() {
    let mut store = store::Store::default();
    let stdin = std::io::stdin();
    print!("rudis> ");
    std::io::stdout().flush().ok();
    for line in stdin.lock().lines() {
        let line = line.unwrap_or_default();
        let args: Vec<Vec<u8>> = line.split_whitespace().map(|s| s.as_bytes().to_vec()).collect();
        if !args.is_empty() {
            let v = commands::dispatch(&mut store, &args, now());
            let mut o = Vec::new();
            resp::encode(&v, &mut o);
            print!("{}", String::from_utf8_lossy(&o));
        }
        print!("rudis> ");
        std::io::stdout().flush().ok();
    }
}

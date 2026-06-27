mod commands;
mod resp;
mod server;
mod store;

use std::sync::{Arc, Mutex};

fn main() {
    let store = Arc::new(Mutex::new(store::Store::default()));
    if let Err(e) = server::serve("127.0.0.1:6380", store) {
        eprintln!("error: {e}");
    }
}

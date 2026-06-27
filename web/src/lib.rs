mod commands;
mod resp;
mod store;

use resp::Value;
use store::Store;
use wasm_bindgen::prelude::*;

// the same command engine, kept in the browser. the page feeds it a line and
// Date.now() (there's no clock on wasm) and gets back a redis-cli-style reply.
#[wasm_bindgen]
pub struct Db {
    store: Store,
}

#[wasm_bindgen]
impl Db {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Db {
        Db { store: Store::default() }
    }

    pub fn exec(&mut self, line: &str, now: f64) -> String {
        let args: Vec<Vec<u8>> = line.split_whitespace().map(|s| s.as_bytes().to_vec()).collect();
        if args.is_empty() {
            return String::new();
        }
        render(&commands::dispatch(&mut self.store, &args, now as u64))
    }

    pub fn stats(&self) -> String {
        let (s, l, h) = self.store.type_counts();
        let total = self.store.hits + self.store.misses;
        let rate = if total > 0 { self.store.hits as f64 / total as f64 * 100.0 } else { 0.0 };
        format!(
            "{{\"keys\":{},\"strings\":{s},\"lists\":{l},\"hashes\":{h},\"cmds\":{},\"rate\":{rate:.0}}}",
            self.store.data.len(),
            self.store.cmds
        )
    }
}

fn render(v: &Value) -> String {
    match v {
        Value::Simple(s) => s.clone(),
        Value::Error(s) => format!("(error) {s}"),
        Value::Int(n) => format!("(integer) {n}"),
        Value::Bulk(b) => format!("\"{}\"", String::from_utf8_lossy(b)),
        Value::Null => "(nil)".into(),
        Value::Array(a) => {
            if a.is_empty() {
                return "(empty array)".into();
            }
            a.iter()
                .enumerate()
                .map(|(i, x)| format!("{}) {}", i + 1, render(x)))
                .collect::<Vec<_>>()
                .join("\n")
        }
    }
}

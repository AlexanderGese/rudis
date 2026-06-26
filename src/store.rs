use std::collections::{HashMap, VecDeque};

pub enum Entry {
    Str(Vec<u8>),
    List(VecDeque<Vec<u8>>),
    Hash(HashMap<Vec<u8>, Vec<u8>>),
}

#[derive(Default)]
pub struct Store {
    pub data: HashMap<Vec<u8>, Entry>,
    pub expires: HashMap<Vec<u8>, u64>, // unix ms deadline
    pub cmds: u64,
    pub hits: u64,
    pub misses: u64,
}

impl Store {
    // drop the key if its TTL has passed (lazy expiry, like redis)
    pub fn expire_check(&mut self, key: &[u8], now: u64) {
        if let Some(&deadline) = self.expires.get(key) {
            if now >= deadline {
                self.data.remove(key);
                self.expires.remove(key);
            }
        }
    }

    pub fn type_name(&self, key: &[u8]) -> &'static str {
        match self.data.get(key) {
            Some(Entry::Str(_)) => "string",
            Some(Entry::List(_)) => "list",
            Some(Entry::Hash(_)) => "hash",
            None => "none",
        }
    }

    pub fn type_counts(&self) -> (usize, usize, usize) {
        let mut c = (0, 0, 0);
        for v in self.data.values() {
            match v {
                Entry::Str(_) => c.0 += 1,
                Entry::List(_) => c.1 += 1,
                Entry::Hash(_) => c.2 += 1,
            }
        }
        c
    }
}

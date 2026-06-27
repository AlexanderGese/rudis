use crate::resp::Value;
use crate::store::{Entry, Store};
use std::collections::{HashMap, VecDeque};

pub fn dispatch(store: &mut Store, args: &[Vec<u8>], now: u64) -> Value {
    if args.is_empty() {
        return Value::err("ERR empty command");
    }
    store.cmds += 1;
    let name = String::from_utf8_lossy(&args[0]).to_uppercase();
    let a = args; // shorthand

    match name.as_str() {
        "PING" => {
            if a.len() > 1 {
                Value::Bulk(a[1].clone())
            } else {
                Value::Simple("PONG".into())
            }
        }
        "ECHO" => arity(a, 2).unwrap_or_else(|| Value::Bulk(a[1].clone())),
        "SET" => set(store, a, now),
        "GET" => get(store, a, now),
        "DEL" => {
            let mut n = 0;
            for k in &a[1..] {
                store.expire_check(k, now);
                if store.data.remove(k).is_some() {
                    store.expires.remove(k);
                    n += 1;
                }
            }
            Value::Int(n)
        }
        "EXISTS" => {
            let mut n = 0;
            for k in &a[1..] {
                store.expire_check(k, now);
                if store.data.contains_key(k) {
                    n += 1;
                }
            }
            Value::Int(n)
        }
        "INCR" => incr_by(store, a, now, 1),
        "DECR" => incr_by(store, a, now, -1),
        "INCRBY" | "DECRBY" => {
            if let Some(e) = arity(a, 3) {
                return e;
            }
            match parse_int(&a[2]) {
                Some(by) => incr_by(store, a, now, if name == "DECRBY" { -by } else { by }),
                None => Value::err("ERR value is not an integer or out of range"),
            }
        }
        "APPEND" => append(store, a, now),
        "STRLEN" => match get_str(store, &a[1], now) {
            Ok(Some(b)) => Value::Int(b.len() as i64),
            Ok(None) => Value::Int(0),
            Err(e) => e,
        },
        "TYPE" => Value::Simple(store.type_name(&a[1]).into()),
        "EXPIRE" => {
            if let Some(e) = arity(a, 3) {
                return e;
            }
            let Some(secs) = parse_int(&a[2]) else {
                return Value::err("ERR value is not an integer or out of range");
            };
            store.expire_check(&a[1], now);
            if store.data.contains_key(&a[1]) {
                store.expires.insert(a[1].clone(), now + (secs.max(0) as u64) * 1000);
                Value::Int(1)
            } else {
                Value::Int(0)
            }
        }
        "TTL" => {
            store.expire_check(&a[1], now);
            if !store.data.contains_key(&a[1]) {
                Value::Int(-2)
            } else if let Some(&dl) = store.expires.get(&a[1]) {
                Value::Int(((dl.saturating_sub(now)) / 1000) as i64)
            } else {
                Value::Int(-1)
            }
        }
        "PERSIST" => Value::Int(store.expires.remove(&a[1]).is_some() as i64),
        "KEYS" => {
            let pat = a.get(1).map(|v| v.as_slice()).unwrap_or(b"*");
            let mut out = Vec::new();
            for k in store.data.keys() {
                if glob(pat, k) {
                    out.push(Value::Bulk(k.clone()));
                }
            }
            Value::Array(out)
        }
        "DBSIZE" => Value::Int(store.data.len() as i64),
        "FLUSHALL" | "FLUSHDB" => {
            store.data.clear();
            store.expires.clear();
            Value::ok()
        }
        "LPUSH" | "RPUSH" => push(store, a, now, name == "LPUSH"),
        "LPOP" | "RPOP" => pop(store, a, now, name == "LPOP"),
        "LLEN" => match store.data.get(a.get(1).map(|v| v.as_slice()).unwrap_or(b"")) {
            Some(Entry::List(l)) => Value::Int(l.len() as i64),
            Some(_) => wrongtype(),
            None => Value::Int(0),
        },
        "LRANGE" => lrange(store, a),
        "HSET" => hset(store, a),
        "HGET" => hget(store, a),
        "HDEL" => hdel(store, a),
        "HGETALL" => hgetall(store, a),
        "HLEN" => match store.data.get(&a[1]) {
            Some(Entry::Hash(h)) => Value::Int(h.len() as i64),
            Some(_) => wrongtype(),
            None => Value::Int(0),
        },
        // things redis-cli fires on connect — keep it happy
        "COMMAND" | "CONFIG" | "CLIENT" => Value::Array(vec![]),
        "QUIT" => Value::ok(),
        "INFO" => Value::Bulk(format!("# rudis\r\nkeys:{}\r\ncommands_processed:{}\r\n", store.data.len(), store.cmds).into_bytes()),
        other => Value::Error(format!("ERR unknown command '{other}'")),
    }
}

fn arity(a: &[Vec<u8>], n: usize) -> Option<Value> {
    if a.len() < n {
        Some(Value::Error(format!(
            "ERR wrong number of arguments for '{}'",
            String::from_utf8_lossy(&a[0]).to_lowercase()
        )))
    } else {
        None
    }
}

fn wrongtype() -> Value {
    Value::err("WRONGTYPE Operation against a key holding the wrong kind of value")
}

fn parse_int(b: &[u8]) -> Option<i64> {
    std::str::from_utf8(b).ok().and_then(|s| s.parse().ok())
}

fn set(store: &mut Store, a: &[Vec<u8>], now: u64) -> Value {
    if let Some(e) = arity(a, 3) {
        return e;
    }
    store.data.insert(a[1].clone(), Entry::Str(a[2].clone()));
    store.expires.remove(&a[1]); // plain SET clears any TTL
    // optional EX <secs> / PX <ms>
    let mut i = 3;
    while i + 1 < a.len() {
        let opt = String::from_utf8_lossy(&a[i]).to_uppercase();
        if let Some(n) = parse_int(&a[i + 1]) {
            match opt.as_str() {
                "EX" => {
                    store.expires.insert(a[1].clone(), now + (n.max(0) as u64) * 1000);
                }
                "PX" => {
                    store.expires.insert(a[1].clone(), now + n.max(0) as u64);
                }
                _ => {}
            }
        }
        i += 2;
    }
    Value::ok()
}

fn get_str(store: &mut Store, key: &[u8], now: u64) -> Result<Option<Vec<u8>>, Value> {
    store.expire_check(key, now);
    match store.data.get(key) {
        Some(Entry::Str(b)) => Ok(Some(b.clone())),
        Some(_) => Err(wrongtype()),
        None => Ok(None),
    }
}

fn get(store: &mut Store, a: &[Vec<u8>], now: u64) -> Value {
    if let Some(e) = arity(a, 2) {
        return e;
    }
    match get_str(store, &a[1], now) {
        Ok(Some(b)) => {
            store.hits += 1;
            Value::Bulk(b)
        }
        Ok(None) => {
            store.misses += 1;
            Value::Null
        }
        Err(e) => e,
    }
}

fn incr_by(store: &mut Store, a: &[Vec<u8>], now: u64, delta: i64) -> Value {
    if let Some(e) = arity(a, 2) {
        return e;
    }
    store.expire_check(&a[1], now);
    let cur = match store.data.get(&a[1]) {
        Some(Entry::Str(b)) => match parse_int(b) {
            Some(n) => n,
            None => return Value::err("ERR value is not an integer or out of range"),
        },
        Some(_) => return wrongtype(),
        None => 0,
    };
    let n = cur + delta;
    store.data.insert(a[1].clone(), Entry::Str(n.to_string().into_bytes()));
    Value::Int(n)
}

fn append(store: &mut Store, a: &[Vec<u8>], now: u64) -> Value {
    if let Some(e) = arity(a, 3) {
        return e;
    }
    store.expire_check(&a[1], now);
    match store.data.entry(a[1].clone()).or_insert_with(|| Entry::Str(Vec::new())) {
        Entry::Str(b) => {
            b.extend_from_slice(&a[2]);
            Value::Int(b.len() as i64)
        }
        _ => wrongtype(),
    }
}

fn push(store: &mut Store, a: &[Vec<u8>], now: u64, left: bool) -> Value {
    if let Some(e) = arity(a, 3) {
        return e;
    }
    store.expire_check(&a[1], now);
    match store.data.entry(a[1].clone()).or_insert_with(|| Entry::List(VecDeque::new())) {
        Entry::List(l) => {
            for v in &a[2..] {
                if left {
                    l.push_front(v.clone());
                } else {
                    l.push_back(v.clone());
                }
            }
            Value::Int(l.len() as i64)
        }
        _ => wrongtype(),
    }
}

fn pop(store: &mut Store, a: &[Vec<u8>], now: u64, left: bool) -> Value {
    if let Some(e) = arity(a, 2) {
        return e;
    }
    store.expire_check(&a[1], now);
    match store.data.get_mut(&a[1]) {
        Some(Entry::List(l)) => {
            let v = if left { l.pop_front() } else { l.pop_back() };
            match v {
                Some(b) => Value::Bulk(b),
                None => Value::Null,
            }
        }
        Some(_) => wrongtype(),
        None => Value::Null,
    }
}

fn lrange(store: &mut Store, a: &[Vec<u8>]) -> Value {
    if let Some(e) = arity(a, 4) {
        return e;
    }
    let (Some(mut start), Some(mut stop)) = (parse_int(&a[2]), parse_int(&a[3])) else {
        return Value::err("ERR value is not an integer or out of range");
    };
    match store.data.get(&a[1]) {
        Some(Entry::List(l)) => {
            let len = l.len() as i64;
            if start < 0 {
                start += len;
            }
            if stop < 0 {
                stop += len;
            }
            start = start.max(0);
            stop = stop.min(len - 1);
            let mut out = Vec::new();
            let mut i = start;
            while i <= stop {
                if let Some(v) = l.get(i as usize) {
                    out.push(Value::Bulk(v.clone()));
                }
                i += 1;
            }
            Value::Array(out)
        }
        Some(_) => wrongtype(),
        None => Value::Array(vec![]),
    }
}

fn hset(store: &mut Store, a: &[Vec<u8>]) -> Value {
    if a.len() < 4 || a.len() % 2 != 0 {
        return arity(a, 4).unwrap_or_else(|| Value::err("ERR wrong number of arguments for 'hset'"));
    }
    let h = match store.data.entry(a[1].clone()).or_insert_with(|| Entry::Hash(HashMap::new())) {
        Entry::Hash(h) => h,
        _ => return wrongtype(),
    };
    let mut added = 0;
    let mut i = 2;
    while i + 1 < a.len() {
        if h.insert(a[i].clone(), a[i + 1].clone()).is_none() {
            added += 1;
        }
        i += 2;
    }
    Value::Int(added)
}

fn hget(store: &mut Store, a: &[Vec<u8>]) -> Value {
    if let Some(e) = arity(a, 3) {
        return e;
    }
    match store.data.get(&a[1]) {
        Some(Entry::Hash(h)) => h.get(&a[2]).map(|v| Value::Bulk(v.clone())).unwrap_or(Value::Null),
        Some(_) => wrongtype(),
        None => Value::Null,
    }
}

fn hdel(store: &mut Store, a: &[Vec<u8>]) -> Value {
    if let Some(e) = arity(a, 3) {
        return e;
    }
    match store.data.get_mut(&a[1]) {
        Some(Entry::Hash(h)) => {
            let mut n = 0;
            for f in &a[2..] {
                if h.remove(f).is_some() {
                    n += 1;
                }
            }
            Value::Int(n)
        }
        Some(_) => wrongtype(),
        None => Value::Int(0),
    }
}

fn hgetall(store: &mut Store, a: &[Vec<u8>]) -> Value {
    match store.data.get(&a[1]) {
        Some(Entry::Hash(h)) => {
            let mut out = Vec::new();
            for (k, v) in h {
                out.push(Value::Bulk(k.clone()));
                out.push(Value::Bulk(v.clone()));
            }
            Value::Array(out)
        }
        Some(_) => wrongtype(),
        None => Value::Array(vec![]),
    }
}

fn glob(pat: &[u8], s: &[u8]) -> bool {
    match (pat.first(), s.first()) {
        (None, None) => true,
        (Some(b'*'), _) => glob(&pat[1..], s) || (!s.is_empty() && glob(pat, &s[1..])),
        (Some(b'?'), Some(_)) => glob(&pat[1..], &s[1..]),
        (Some(x), Some(y)) if x == y => glob(&pat[1..], &s[1..]),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(store: &mut Store, parts: &[&str]) -> Value {
        let args: Vec<Vec<u8>> = parts.iter().map(|s| s.as_bytes().to_vec()).collect();
        dispatch(store, &args, 1_000_000)
    }

    #[test]
    fn strings_and_counters() {
        let mut s = Store::default();
        assert_eq!(cmd(&mut s, &["SET", "x", "10"]), Value::ok());
        assert_eq!(cmd(&mut s, &["GET", "x"]), Value::bulk("10"));
        assert_eq!(cmd(&mut s, &["INCR", "x"]), Value::Int(11));
        assert_eq!(cmd(&mut s, &["INCRBY", "x", "5"]), Value::Int(16));
        assert_eq!(cmd(&mut s, &["GET", "missing"]), Value::Null);
    }

    #[test]
    fn lists_and_hashes() {
        let mut s = Store::default();
        assert_eq!(cmd(&mut s, &["RPUSH", "l", "a", "b", "c"]), Value::Int(3));
        assert_eq!(cmd(&mut s, &["LPOP", "l"]), Value::bulk("a"));
        assert_eq!(cmd(&mut s, &["LLEN", "l"]), Value::Int(2));
        assert_eq!(cmd(&mut s, &["HSET", "h", "f", "1"]), Value::Int(1));
        assert_eq!(cmd(&mut s, &["HGET", "h", "f"]), Value::bulk("1"));
        assert_eq!(cmd(&mut s, &["TYPE", "h"]), Value::Simple("hash".into()));
    }

    #[test]
    fn expiry() {
        let mut s = Store::default();
        cmd(&mut s, &["SET", "k", "v"]);
        let args: Vec<Vec<u8>> = ["EXPIRE", "k", "10"].iter().map(|s| s.as_bytes().to_vec()).collect();
        assert_eq!(dispatch(&mut s, &args, 0), Value::Int(1));
        // 5s in: still there
        let g: Vec<Vec<u8>> = ["GET", "k"].iter().map(|s| s.as_bytes().to_vec()).collect();
        assert_eq!(dispatch(&mut s, &g, 5_000), Value::bulk("v"));
        // 11s in: gone
        assert_eq!(dispatch(&mut s, &g, 11_000), Value::Null);
    }
}

// RESP - the wire format redis-cli speaks. We parse requests (an array of bulk
// strings, or an inline line) and serialise replies.

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Simple(String),
    Error(String),
    Int(i64),
    Bulk(Vec<u8>),
    Null,
    Array(Vec<Value>),
}

impl Value {
    pub fn ok() -> Value {
        Value::Simple("OK".into())
    }
    pub fn err(msg: &str) -> Value {
        Value::Error(msg.into())
    }
    pub fn bulk(s: impl Into<Vec<u8>>) -> Value {
        Value::Bulk(s.into())
    }
}

pub fn encode(v: &Value, out: &mut Vec<u8>) {
    match v {
        Value::Simple(s) => line(out, b'+', s.as_bytes()),
        Value::Error(s) => line(out, b'-', s.as_bytes()),
        Value::Int(n) => line(out, b':', n.to_string().as_bytes()),
        Value::Bulk(b) => {
            line(out, b'$', b.len().to_string().as_bytes());
            out.extend_from_slice(b);
            out.extend_from_slice(b"\r\n");
        }
        Value::Null => out.extend_from_slice(b"$-1\r\n"),
        Value::Array(a) => {
            line(out, b'*', a.len().to_string().as_bytes());
            for x in a {
                encode(x, out);
            }
        }
    }
}

fn line(out: &mut Vec<u8>, tag: u8, body: &[u8]) {
    out.push(tag);
    out.extend_from_slice(body);
    out.extend_from_slice(b"\r\n");
}

fn read_line(buf: &[u8], pos: usize) -> Option<(&[u8], usize)> {
    let nl = buf[pos..].iter().position(|&b| b == b'\n')? + pos;
    let end = if nl > pos && buf[nl - 1] == b'\r' { nl - 1 } else { nl };
    Some((&buf[pos..end], nl + 1))
}

// Pull one command off the front of `buf`. Ok(None) means "need more bytes".
// Returns the args and how many bytes were consumed.
pub fn parse_command(buf: &[u8]) -> Result<Option<(Vec<Vec<u8>>, usize)>, String> {
    if buf.is_empty() {
        return Ok(None);
    }
    if buf[0] != b'*' {
        // inline command (handy over telnet)
        let Some((line, next)) = read_line(buf, 0) else {
            return Ok(None);
        };
        let args = line.split(|&b| b == b' ').filter(|s| !s.is_empty()).map(<[u8]>::to_vec).collect();
        return Ok(Some((args, next)));
    }

    let mut pos = 0;
    let Some((hdr, p)) = read_line(buf, pos) else {
        return Ok(None);
    };
    pos = p;
    let count: i64 = std::str::from_utf8(&hdr[1..])
        .ok()
        .and_then(|s| s.parse().ok())
        .ok_or("bad multibulk length")?;

    let mut args = Vec::new();
    for _ in 0..count.max(0) {
        if pos >= buf.len() {
            return Ok(None);
        }
        if buf[pos] != b'$' {
            return Err("expected a bulk string".into());
        }
        let Some((bh, p2)) = read_line(buf, pos) else {
            return Ok(None);
        };
        let len: usize = std::str::from_utf8(&bh[1..])
            .ok()
            .and_then(|s| s.parse().ok())
            .ok_or("bad bulk length")?;
        pos = p2;
        if pos + len + 2 > buf.len() {
            return Ok(None);
        }
        args.push(buf[pos..pos + len].to_vec());
        pos += len + 2;
    }
    Ok(Some((args, pos)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_replies() {
        let mut o = Vec::new();
        encode(&Value::ok(), &mut o);
        assert_eq!(o, b"+OK\r\n");
        o.clear();
        encode(&Value::Int(7), &mut o);
        assert_eq!(o, b":7\r\n");
        o.clear();
        encode(&Value::bulk("hi"), &mut o);
        assert_eq!(o, b"$2\r\nhi\r\n");
        o.clear();
        encode(&Value::Null, &mut o);
        assert_eq!(o, b"$-1\r\n");
    }

    #[test]
    fn parses_resp_and_inline() {
        let (a, n) = parse_command(b"*2\r\n$3\r\nGET\r\n$3\r\nfoo\r\n").unwrap().unwrap();
        assert_eq!(a, vec![b"GET".to_vec(), b"foo".to_vec()]);
        assert_eq!(n, 22);

        let (a, _) = parse_command(b"PING\r\n").unwrap().unwrap();
        assert_eq!(a, vec![b"PING".to_vec()]);

        // incomplete -> None
        assert!(parse_command(b"*2\r\n$3\r\nGE").unwrap().is_none());
    }
}

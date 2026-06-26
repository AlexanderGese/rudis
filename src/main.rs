mod resp;

fn main() {
    // smoke check the encoder
    let mut o = Vec::new();
    resp::encode(&resp::Value::Simple("PONG".into()), &mut o);
    print!("{}", String::from_utf8_lossy(&o));
}

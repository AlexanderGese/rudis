mod resp;
mod store;

fn main() {
    let s = store::Store::default();
    println!("rudis store ready ({} keys)", s.data.len());
}

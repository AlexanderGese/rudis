# rudis

A small Redis-compatible server written from scratch in Rust — the RESP
protocol, an in-memory keyspace with strings / lists / hashes, key expiry, and
about thirty commands. `redis-cli` talks to it without knowing the difference.

There's an in-browser REPL too (the same command engine, compiled to wasm):
**https://alexandergese.github.io/rudis/**

## What works

- **RESP** — parses the same wire format `redis-cli` speaks (and inline commands
  over telnet), streams replies back.
- **Types** — strings, lists (`LPUSH`/`RPUSH`/`LPOP`/`LRANGE`…), hashes
  (`HSET`/`HGET`/`HGETALL`…).
- **Expiry** — `EXPIRE` / `TTL` / `PERSIST`, lazily evicted on access.
- **The usual** — `GET`/`SET` (with `EX`/`PX`), `INCR`/`DECR`/`INCRBY`,
  `APPEND`, `DEL`, `EXISTS`, `KEYS` (glob), `TYPE`, `DBSIZE`, `FLUSHALL`, …
- **A TUI** — an interactive REPL with a live keyspace dashboard (key counts by
  type, commands processed, hit-rate).

## Use it

```
rudis                  # the interactive REPL + dashboard (default)
rudis serve            # RESP server on 127.0.0.1:6380
rudis serve -p 6400    # ...on another port

# then, from anywhere:
redis-cli -p 6380 set hello world
redis-cli -p 6380 get hello
```

## Build

```
cargo build --release
cargo test
```

## License

MIT

mod commands;
mod resp;
mod server;
mod store;
mod tui;

use clap::{Parser, Subcommand};
use std::process::ExitCode;
use std::sync::{Arc, Mutex};

#[derive(Parser)]
#[command(name = "rudis", version, about = "A small Redis-compatible server")]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run the RESP server (works with redis-cli)
    Serve {
        #[arg(short, long, default_value_t = 6380)]
        port: u16,
    },
    /// Interactive REPL + live keyspace dashboard (the default)
    Tui,
}

fn main() -> ExitCode {
    let cmd = Cli::parse().cmd.unwrap_or(Cmd::Tui);
    match cmd {
        Cmd::Serve { port } => {
            let store = Arc::new(Mutex::new(store::Store::default()));
            if let Err(e) = server::serve(&format!("127.0.0.1:{port}"), store) {
                eprintln!("\x1b[31merror:\x1b[0m {e}");
                return ExitCode::FAILURE;
            }
        }
        Cmd::Tui => {
            if let Err(e) = tui::run() {
                eprintln!("\x1b[31mtui error:\x1b[0m {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    ExitCode::SUCCESS
}

//! The `parceltrack` command line.

use std::process::ExitCode;

use parceltrack::config::Config;
use parceltrack::model::Parcel;
use parceltrack::notify::{notify_with_retry, Sender};
use parceltrack::server;
use parceltrack::store::Store;

const USAGE: &str = "usage: parceltrack <command>\n\
  add <id> <destination>   register a parcel\n\
  status <id>              show one parcel\n\
  advance <id>             move a parcel to its next status\n\
  return <id>              mark a parcel as returned to its sender\n\
  list                     show every parcel\n\
  serve                    answer status lookups over HTTP\n";

/// Notifications go to the standard error stream.
struct StderrSender;

impl Sender for StderrSender {
    fn send(&mut self, message: &str) -> Result<(), String> {
        eprintln!("notify: {message}");
        Ok(())
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let config = Config::from_env();
    match run(&args, &config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Usage) => {
            eprint!("{USAGE}");
            ExitCode::from(2)
        }
        Err(Failure::Runtime(message)) => {
            eprintln!("parceltrack: {message}");
            ExitCode::from(1)
        }
    }
}

enum Failure {
    Usage,
    Runtime(String),
}

impl<E: std::error::Error> From<E> for Failure {
    fn from(error: E) -> Self {
        Failure::Runtime(error.to_string())
    }
}

fn run(args: &[String], config: &Config) -> Result<(), Failure> {
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["add", id, destination] => {
            let mut store = Store::open(&config.data_file)?;
            store.insert(Parcel::new(id, destination)?)?;
            println!("registered {id}");
        }
        ["status", id] => {
            let store = Store::open(&config.data_file)?;
            let parcel = store
                .get(id)
                .ok_or_else(|| Failure::Runtime(format!("no parcel {id}")))?;
            println!("{}\t{}\t{}", parcel.id, parcel.status, parcel.destination);
        }
        ["advance", id] => {
            let mut store = Store::open(&config.data_file)?;
            let status = store.advance(id)?;
            println!("{id}\t{status}");
            let message = format!("parcel {id} is now {status}");
            if let Err(error) = notify_with_retry(&mut StderrSender, &message, std::thread::sleep) {
                eprintln!("parceltrack: {error}");
            }
        }
        ["return", id] => {
            let mut store = Store::open(&config.data_file)?;
            store.mark_returned(id)?;
            println!("{id}\treturned");
        }
        ["list"] => {
            let store = Store::open(&config.data_file)?;
            for parcel in store.all() {
                println!("{}\t{}\t{}", parcel.id, parcel.status, parcel.destination);
            }
        }
        ["serve"] => server::serve(config)?,
        _ => return Err(Failure::Usage),
    }
    Ok(())
}

use std::env;
use std::io::Write as _;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use openworks_server::{Issuer, Server, Store};

const USAGE: &str = "usage: openworks-server migrate | init <org name> <owner email> | serve";

fn main() -> ExitCode {
    let words: Vec<String> = env::args().skip(1).collect();
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    match run(&words) {
        Ok(said) => {
            writeln!(std::io::stdout(), "{said}").ok();
            ExitCode::SUCCESS
        }
        Err(why) => {
            writeln!(std::io::stderr(), "{why}").ok();
            ExitCode::FAILURE
        }
    }
}

fn run(words: &[&str]) -> Result<String, String> {
    let store = Store::connect(
        &setting("OPENWORKS_DATABASE_URL")?,
        PathBuf::from(or("OPENWORKS_FILES", "files")),
    )
    .map_err(|fail| fail.to_string())?;
    match words {
        ["migrate"] => store
            .migrate()
            .map(|()| "migrated".to_owned())
            .map_err(|fail| fail.to_string()),
        ["init", name, owner] => server(&store)?
            .init(name, owner)
            .map(|org| org.0)
            .map_err(|fail| fail.to_string()),
        ["serve"] => {
            let address = or("OPENWORKS_ADDRESS", "127.0.0.1:8080");
            let listener = TcpListener::bind(&address).map_err(|error| format!("{address}: {error}"))?;
            let every = or("OPENWORKS_DUE_SECONDS", "60")
                .parse()
                .map_err(|_| "OPENWORKS_DUE_SECONDS is a whole number of seconds".to_owned())?;
            server(&store)?.serve(listener, Duration::from_secs(every));
            Ok("stopped".to_owned())
        }
        _ => Err(USAGE.to_owned()),
    }
}

fn server(store: &Store) -> Result<Server<'_>, String> {
    let issuer = Issuer::discover(&setting("OPENWORKS_ISSUER")?, &setting("OPENWORKS_CLIENT_ID")?)?;
    Server::new(store, issuer, PathBuf::from(or("OPENWORKS_WEB", "web"))).map_err(|refused| format!("{refused:?}"))
}

fn setting(name: &str) -> Result<String, String> {
    env::var(name).map_err(|_| format!("set {name}"))
}

fn or(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_owned())
}

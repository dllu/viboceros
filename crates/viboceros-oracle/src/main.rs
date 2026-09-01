use std::env;

fn main() {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.len() != 2 {
        eprintln!("usage: viboceros-oracle REQUEST.json RESPONSE.json");
        std::process::exit(2);
    }
    if let Err(error) = viboceros_oracle::run_files(&arguments[0], &arguments[1]) {
        eprintln!("oracle probe failed: {error}");
        std::process::exit(1);
    }
}

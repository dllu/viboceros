use std::env;

fn main() {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    let result = match arguments.as_slice() {
        [input, output] => viboceros_oracle::run_files(input, output),
        [flag, input, output] if flag == "--audit" => {
            viboceros_oracle::run_audit_files(input, output)
        }
        _ => {
            eprintln!("usage: viboceros-oracle [--audit] REQUEST.json RESPONSE.json");
            std::process::exit(2);
        }
    };
    if let Err(error) = result {
        eprintln!("oracle probe failed: {error}");
        std::process::exit(1);
    }
}

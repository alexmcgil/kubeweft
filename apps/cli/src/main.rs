use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut arguments = env::args_os();
    let _program = arguments.next();
    let command = arguments.next();

    if let Some(trailing) = arguments.next() {
        eprintln!("unknown command: {}", trailing.to_string_lossy());
        return ExitCode::from(2);
    }

    match command {
        Some(command) if command == "--version" => {
            println!("kubeweft {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some(command) if command == "doctor" => {
            println!("cluster connectivity and platform capabilities are placeholders");
            ExitCode::SUCCESS
        }
        Some(command) => {
            eprintln!("unknown command: {}", command.to_string_lossy());
            ExitCode::from(2)
        }
        None => {
            eprintln!("unknown command: ");
            ExitCode::from(2)
        }
    }
}

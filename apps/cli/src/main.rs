use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut arguments = env::args_os();
    let _program = arguments.next();

    match arguments.next() {
        Some(command) if command == "--version" => {
            println!("kubeweft 0.1.0");
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

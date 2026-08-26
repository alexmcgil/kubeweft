use std::process::ExitCode;

fn main() -> ExitCode {
    println!("kubeweft-agent: platform adapters are not configured");
    ExitCode::SUCCESS
}

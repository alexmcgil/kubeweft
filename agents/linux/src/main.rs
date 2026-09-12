use std::{env, path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("kubeweft-agent: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: Vec<String>) -> Result<(), String> {
    if arguments.as_slice() == ["--version"] {
        println!("kubeweft-agent {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if arguments.as_slice() == ["--help"] {
        print_help();
        return Ok(());
    }

    let mut config = kubeweft_transport::AgentConfig::new(default_data_directory()?);
    let mut index = 0;
    while index < arguments.len() {
        let option = &arguments[index];
        index += 1;
        let value = arguments
            .get(index)
            .ok_or_else(|| format!("missing value for {option}"));
        match option.as_str() {
            "--data-dir" => {
                config.data_directory = PathBuf::from(value?);
                index += 1;
            }
            "--listen" => {
                config.listen = parse(value?, "listen endpoint")?;
                index += 1;
            }
            "--advertise" => {
                config.advertise = Some(parse(value?, "advertised endpoint")?);
                index += 1;
            }
            "--discovery-port" => {
                config.discovery_port = Some(parse(value?, "discovery port")?);
                index += 1;
            }
            "--name" => {
                config.device_name = Some(value?.to_owned());
                index += 1;
            }
            "--no-discovery" => config.discovery_port = None,
            _ => return Err(format!("unknown option: {option}")),
        }
    }

    let discovery = config
        .discovery_port
        .map_or_else(|| "disabled".to_owned(), |port| port.to_string());
    let handle =
        kubeweft_transport::AgentHandle::start(config).map_err(|error| error.to_string())?;
    println!("kubeweft-agent listening={}", handle.endpoint());
    println!("advertising={}", handle.advertised_endpoint());
    println!("discovery={discovery}");
    handle.wait().map_err(|error| error.to_string())
}

fn default_data_directory() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("KUBEWEFT_DATA_DIR") {
        return Ok(path.into());
    }
    let home = env::var_os("HOME")
        .ok_or_else(|| "HOME is unset; pass --data-dir or set KUBEWEFT_DATA_DIR".to_owned())?;
    Ok(PathBuf::from(home).join(".local/share/kubeweft"))
}

fn parse<T: std::str::FromStr>(value: &str, name: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("invalid {name}: {value}"))
}

fn print_help() {
    println!("kubeweft-agent [OPTIONS]");
    println!("  --data-dir DIR          persistent local state");
    println!("  --listen ADDRESS        TCP listen address (default 127.0.0.1:37846)");
    println!("  --advertise ADDRESS     reachable address announced to peers");
    println!("  --discovery-port PORT   LAN discovery port (default 37845)");
    println!("  --no-discovery          disable LAN discovery");
    println!("  --name NAME             device display name on first start");
    println!("  peer TCP uses an authenticated encrypted Noise channel");
}

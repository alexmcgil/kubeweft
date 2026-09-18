use std::{
    env,
    ffi::OsString,
    io::{self, Read},
    path::PathBuf,
    process::ExitCode,
    time::Duration,
};

use kubeweft_filesystem::{EntryMetadata, FileType, FilesystemError};
use kubeweft_local::LocalFilesystem;
use kubeweft_transport::{ClusterRole, LocalControlClient, TransportError, discover};

fn main() -> ExitCode {
    match run(env::args_os().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(CliError::Usage(message)) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
        Err(CliError::Filesystem(error)) => {
            eprintln!("filesystem error: {error}");
            ExitCode::FAILURE
        }
        Err(CliError::Io(error)) => {
            eprintln!("I/O error: {error}");
            ExitCode::FAILURE
        }
        Err(CliError::Transport(error)) => {
            eprintln!("transport error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(mut arguments: Vec<OsString>) -> Result<(), CliError> {
    if arguments.as_slice() == ["--version"] {
        println!("kubeweft {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if arguments.as_slice() == ["--help"] || arguments.is_empty() {
        print_help();
        return Ok(());
    }
    if arguments.as_slice() == ["doctor"] {
        println!("local filesystem: ready");
        println!("cluster connectivity: available through kubeweft-agent");
        println!("platform capabilities: not configured");
        return Ok(());
    }

    let data_directory = take_data_directory(&mut arguments)?;
    let command_group = take_utf8(&mut arguments, "command")?;
    match command_group.as_str() {
        "fs" => run_filesystem(arguments, data_directory),
        "cluster" => run_cluster(arguments, data_directory),
        _ => Err(CliError::Usage(format!("unknown command: {command_group}"))),
    }
}

fn run_filesystem(mut arguments: Vec<OsString>, data_directory: PathBuf) -> Result<(), CliError> {
    let command = take_utf8(&mut arguments, "filesystem command")?;
    let local = LocalFilesystem::open(data_directory)?;

    match command.as_str() {
        "init" => {
            let user = optional_utf8(&mut arguments)?.unwrap_or_else(|| "user".to_owned());
            ensure_empty(&arguments)?;
            println!("{}", local.bootstrap_home(&user)?);
        }
        "mkdir" => local.service().mkdir(&single_path(&mut arguments)?)?,
        "create" => {
            let metadata = local.service().create(&single_path(&mut arguments)?)?;
            println!("{}", metadata.generation);
        }
        "ls" => {
            let path = optional_utf8(&mut arguments)?.unwrap_or_else(|| "/".to_owned());
            ensure_empty(&arguments)?;
            for entry in local.service().list(&path)? {
                let kind = match entry.metadata.file_type() {
                    FileType::File => "file",
                    FileType::Directory => "directory",
                };
                println!("{kind}\t{}\t{}", entry.metadata.generation(), entry.name);
            }
        }
        "stat" => print_stat(local.service().stat(&single_path(&mut arguments)?)?),
        "cat" => {
            let data = local.service().read(&single_path(&mut arguments)?)?;
            io::Write::write_all(&mut io::stdout().lock(), &data)?;
        }
        "write" => {
            let path = take_utf8(&mut arguments, "path")?;
            let expected = take_expected_generation(&mut arguments)?;
            ensure_empty(&arguments)?;
            let mut data = Vec::new();
            io::stdin().read_to_end(&mut data)?;
            let metadata = local
                .service()
                .write(&path, &data, expected, local.device_id())?;
            println!("{}", metadata.generation);
        }
        "mv" => {
            let from = take_utf8(&mut arguments, "source path")?;
            let to = take_utf8(&mut arguments, "destination path")?;
            ensure_empty(&arguments)?;
            local.service().rename(&from, &to)?;
        }
        "rm" => local.service().remove(&single_path(&mut arguments)?)?,
        _ => {
            return Err(CliError::Usage(format!(
                "unknown filesystem command: {command}"
            )));
        }
    }
    Ok(())
}

fn run_cluster(mut arguments: Vec<OsString>, data_directory: PathBuf) -> Result<(), CliError> {
    let command = take_utf8(&mut arguments, "cluster command")?;
    if command == "discover" {
        let discovery_port = take_named_u16(&mut arguments, "--port")?.unwrap_or(37_845);
        let timeout = take_named_u64(&mut arguments, "--timeout-ms")?.unwrap_or(1_200);
        ensure_empty(&arguments)?;
        for instance in discover(discovery_port, Duration::from_millis(timeout))? {
            let cluster = instance
                .cluster
                .map_or_else(|| "-".to_owned(), |cluster| cluster.name);
            println!(
                "{}\t{}\t{}\t{}",
                instance.device_id, instance.device_name, instance.endpoint, cluster
            );
        }
        return Ok(());
    }

    let client = LocalControlClient::local(data_directory)?;
    match command.as_str() {
        "status" => {
            ensure_empty(&arguments)?;
            let status = client.status()?;
            println!("device\t{}\t{}", status.device_id, status.device_name);
            match (status.cluster, status.role) {
                (Some(cluster), Some(role)) => println!(
                    "cluster\t{}\t{}\t{}\t{}",
                    cluster.id,
                    cluster.name,
                    cluster.coordinator,
                    role_name(role)
                ),
                _ => println!("cluster\t-"),
            }
        }
        "create" => {
            let name = take_utf8(&mut arguments, "cluster name")?;
            ensure_empty(&arguments)?;
            let (cluster, invite_token) = client.create_cluster(name)?;
            println!("cluster\t{}\t{}", cluster.id, cluster.name);
            println!("coordinator\t{}", cluster.coordinator);
            println!("pairing\t{invite_token}");
        }
        "invite" => {
            ensure_empty(&arguments)?;
            println!("pairing\t{}", client.create_invite()?);
        }
        "join" => {
            let coordinator = take_utf8(&mut arguments, "coordinator endpoint")?
                .parse()
                .map_err(|_| CliError::Usage("invalid coordinator endpoint".into()))?;
            let invite_token = take_utf8(&mut arguments, "pairing code")?;
            ensure_empty(&arguments)?;
            let cluster = client.join_cluster(coordinator, invite_token)?;
            println!("cluster\t{}\t{}", cluster.id, cluster.name);
            println!("coordinator\t{}", cluster.coordinator);
        }
        "members" => {
            ensure_empty(&arguments)?;
            let (cluster, members) = client.members()?;
            println!("cluster\t{}\t{}", cluster.id, cluster.name);
            for member in members {
                let role = if member.endpoint == cluster.coordinator {
                    "coordinator"
                } else {
                    "member"
                };
                println!(
                    "{}\t{}\t{}\t{}",
                    member.device_id, member.device_name, member.endpoint, role
                );
            }
        }
        "presence" => {
            ensure_empty(&arguments)?;
            for record in client.presence()? {
                println!(
                    "{}\t{}\t{}\t{}",
                    record.member.device_id,
                    record.member.device_name,
                    record.member.endpoint,
                    presence_state_name(record.state)
                );
            }
        }
        _ => {
            return Err(CliError::Usage(format!(
                "unknown cluster command: {command}"
            )));
        }
    }
    Ok(())
}

fn role_name(role: ClusterRole) -> &'static str {
    match role {
        ClusterRole::Coordinator => "coordinator",
        ClusterRole::Member => "member",
    }
}

fn presence_state_name(state: kubeweft_transport::PresenceState) -> &'static str {
    match state {
        kubeweft_transport::PresenceState::Unknown => "unknown",
        kubeweft_transport::PresenceState::Online => "online",
        kubeweft_transport::PresenceState::Offline => "offline",
    }
}

fn take_named_u16(arguments: &mut Vec<OsString>, name: &str) -> Result<Option<u16>, CliError> {
    take_named_u64(arguments, name)?
        .map(|value| {
            value
                .try_into()
                .map_err(|_| CliError::Usage(format!("invalid value for {name}")))
        })
        .transpose()
}

fn take_named_u64(arguments: &mut Vec<OsString>, name: &str) -> Result<Option<u64>, CliError> {
    let Some(index) = arguments.iter().position(|argument| argument == name) else {
        return Ok(None);
    };
    arguments.remove(index);
    if index >= arguments.len() {
        return Err(CliError::Usage(format!("missing value for {name}")));
    }
    take_utf8_at(arguments, index, name)?
        .parse()
        .map(Some)
        .map_err(|_| CliError::Usage(format!("invalid value for {name}")))
}

fn take_utf8_at(
    arguments: &mut Vec<OsString>,
    index: usize,
    name: &str,
) -> Result<String, CliError> {
    arguments
        .remove(index)
        .into_string()
        .map_err(|_| CliError::Usage(format!("{name} must be valid UTF-8")))
}

fn take_data_directory(arguments: &mut Vec<OsString>) -> Result<PathBuf, CliError> {
    if arguments
        .first()
        .is_some_and(|argument| argument == "--data-dir")
    {
        arguments.remove(0);
        return Ok(PathBuf::from(take_os(arguments, "data directory")?));
    }
    if let Some(path) = env::var_os("KUBEWEFT_DATA_DIR") {
        return Ok(path.into());
    }
    let home = env::var_os("HOME").ok_or_else(|| {
        CliError::Usage("HOME is unset; pass --data-dir or set KUBEWEFT_DATA_DIR".into())
    })?;
    Ok(PathBuf::from(home).join(".local/share/kubeweft"))
}

fn take_expected_generation(arguments: &mut Vec<OsString>) -> Result<Option<u64>, CliError> {
    if arguments
        .first()
        .is_none_or(|argument| argument != "--expect")
    {
        return Ok(None);
    }
    arguments.remove(0);
    take_utf8(arguments, "generation")?
        .parse()
        .map(Some)
        .map_err(|_| CliError::Usage("generation must be an unsigned integer".into()))
}

fn print_stat(metadata: EntryMetadata) {
    match metadata {
        EntryMetadata::File(file) => println!("file\t{}\t{}", file.generation, file.size),
        EntryMetadata::Directory(directory) => {
            println!("directory\t{}\t0", directory.generation)
        }
    }
}

fn single_path(arguments: &mut Vec<OsString>) -> Result<String, CliError> {
    let path = take_utf8(arguments, "path")?;
    ensure_empty(arguments)?;
    Ok(path)
}

fn optional_utf8(arguments: &mut Vec<OsString>) -> Result<Option<String>, CliError> {
    if arguments.is_empty() {
        Ok(None)
    } else {
        take_utf8(arguments, "argument").map(Some)
    }
}

fn take_utf8(arguments: &mut Vec<OsString>, name: &str) -> Result<String, CliError> {
    take_os(arguments, name)?
        .into_string()
        .map_err(|_| CliError::Usage(format!("{name} must be valid UTF-8")))
}

fn take_os(arguments: &mut Vec<OsString>, name: &str) -> Result<OsString, CliError> {
    if arguments.is_empty() {
        Err(CliError::Usage(format!("missing {name}")))
    } else {
        Ok(arguments.remove(0))
    }
}

fn ensure_empty(arguments: &[OsString]) -> Result<(), CliError> {
    if let Some(argument) = arguments.first() {
        return Err(CliError::Usage(format!(
            "unexpected argument: {}",
            argument.to_string_lossy()
        )));
    }
    Ok(())
}

fn print_help() {
    println!("kubeweft [--data-dir DIR] <fs|cluster> <command>");
    println!("filesystem: init [USER], mkdir PATH, create PATH, ls [PATH], stat PATH");
    println!("          cat PATH, write PATH [--expect GENERATION], mv FROM TO, rm PATH");
    println!(
        "cluster:  status, create NAME, invite, join ENDPOINT PAIRING_CODE, members, presence"
    );
    println!("          discover [--port PORT] [--timeout-ms MILLISECONDS]");
}

enum CliError {
    Usage(String),
    Filesystem(FilesystemError),
    Transport(TransportError),
    Io(io::Error),
}

impl From<FilesystemError> for CliError {
    fn from(error: FilesystemError) -> Self {
        Self::Filesystem(error)
    }
}

impl From<io::Error> for CliError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<TransportError> for CliError {
    fn from(error: TransportError) -> Self {
        Self::Transport(error)
    }
}

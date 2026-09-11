use std::{
    env,
    ffi::OsString,
    io::{self, Read},
    path::PathBuf,
    process::ExitCode,
};

use kubeweft_filesystem::{EntryMetadata, FileType, FilesystemError};
use kubeweft_local::LocalFilesystem;

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
        println!("cluster connectivity: not configured");
        println!("platform capabilities: not configured");
        return Ok(());
    }

    let data_directory = take_data_directory(&mut arguments)?;
    if arguments.first().is_none_or(|argument| argument != "fs") {
        return Err(CliError::Usage(format!(
            "unknown command: {}",
            arguments
                .first()
                .map_or_else(String::new, |value| value.to_string_lossy().into_owned())
        )));
    }
    arguments.remove(0);
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
    println!("kubeweft [--data-dir DIR] fs <command>");
    println!("commands: init [USER], mkdir PATH, create PATH, ls [PATH], stat PATH");
    println!("          cat PATH, write PATH [--expect GENERATION], mv FROM TO, rm PATH");
}

enum CliError {
    Usage(String),
    Filesystem(FilesystemError),
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

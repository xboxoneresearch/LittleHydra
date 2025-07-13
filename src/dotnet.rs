use crate::assets::DOTNET_PROJ;
use crate::error::Error;
use std::fs;
use std::io::PipeWriter;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use tempfile::TempDir;

pub fn load_dotnet_assembly(
    dotnet_path: &str,
    assembly_path: &str,
    arguments: Option<&str>,
    working_dir: &str,
    output_writer: PipeWriter,
) -> Result<Child, Error> {
    // Create a temporary directory for the build files
    let mut temp_dir = TempDir::new()
        .map_err(|e| Error::ProcessCreation(format!("Failed to create temp directory: {e}")))?;
    temp_dir.disable_cleanup(true);

    let temp_path = temp_dir.path();

    // Write the bundled project file
    let project_path = temp_path.join("AssemblyLoadTask.proj");
    fs::write(&project_path, DOTNET_PROJ)
        .map_err(|e| Error::ProcessCreation(format!("Failed to write project file: {e}")))?;

    // Build the dotnet path
    let dotnet_exe = if dotnet_path.ends_with("dotnet.exe") {
        dotnet_path.to_string()
    } else {
        Path::new(dotnet_path)
            .join("dotnet.exe")
            .to_string_lossy()
            .to_string()
    };

    // Prepare the msbuild command
    let mut command = Command::new(&dotnet_exe);
    command
        .arg("msbuild")
        .arg(&project_path)
        .arg(format!("/p:AssemblyPath={assembly_path}"))
        .current_dir(working_dir)
        .env("DOTNET_CLI_TELEMETRY_OPTOUT", "1")
        .env("DOTNET_EnableWriteXorExecute", "0")
        .env("DOTNET_NOLOGO", "1")
        .env("DOTNET_ROLL_FORWARD", "LatestMajor")
        .stdin(Stdio::null())
        .stdout(output_writer.try_clone()?)
        .stderr(output_writer.try_clone()?);

    // Add arguments if provided
    if let Some(args) = arguments {
        command.arg(format!("/p:Arguments={args}"));
    }
    if !working_dir.is_empty() {
        command.arg(format!("/p:WorkingDirectory={working_dir}"));
    }

    // Execute the command
    let child = command
        .spawn()
        .map_err(|e| Error::ProcessCreation(format!("Failed to execute dotnet msbuild: {e}")))?;

    Ok(child)
}

pub fn load_dotnet_assembly_with_config(
    config: &crate::config::Config,
    assembly_path: &str,
    arguments: Option<&str>,
    working_dir: &str,
    output_writer: PipeWriter,
) -> Result<Child, Error> {
    load_dotnet_assembly(
        &config.general.dotnet_path,
        assembly_path,
        arguments,
        working_dir,
        output_writer,
    )
}

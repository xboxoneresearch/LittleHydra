# Little Hydra

Little Hydra is a modular Windows process manager daemon written in Rust. It allows you to configure, start, stop, and monitor multiple types of services via a TOML config file and a named pipe RPC interface.
It implements ideas / mechanisms from the following projects:

- [Interop](https://github.com/XboxOneResearch/Interop) - Loading unsigned managed code via `dotnet msbuild` 
- [PE code injection](https://github.com/exploits-forsale/solstice) - Loading PE into memory of a foreign process
- [Silverton](https://github.com/kwsimons/Silverton) and [SharpShell](https://github.com/xboxoneresearch/SharpShell) - Powershell execution

## Features
- **Configurable via TOML**: Define services with start priority, working directory, restart policy, and more.
- **Supports multiple execution types**:
  - `cmd`/`bat` scripts (via `cmd.exe`)
  - PowerShell scripts (`ps1`, via `pwsh.exe`)
  - .NET assemblies (via MSBuild task and `dotnet.exe`)
  - MSBuild tasks (via `dotnet.exe`)
  - Native PE executables (reflective PE loading via Solstice shellcode)
- **Service management**: Add, Remove, Start, stop, and query services by name and config file saving.
- **Logging**: Logs to file, stdout and optionally to a remote TCP log host.
- **RPC**: Control and query the manager at runtime, either via named pipe or tcp connection.

## Status

- ✅ Config loading / saving
- ✅ CMD / bat execution
- ❌ Powershell scripts - Needs implementation of SharpShell / Silverton Assemby.Load() mechanism
- ✅ .NET assemblies
- ✅ MSBuild tasks
- ✅ Native PE executable injection
- ❌ Argument passing in general - Not tested fully
- ❌ Making use of "working_directory" parameter

## Configuration

Check out `config.toml` file in the project root.

## Building

- Requires Rust (latest stable).

### Windows

```
cargo build --release --target x86_64-pc-windows-msvc
```

### Linux

```
cargo install xwin
cargo xwin build --release --target x86_64-pc-windows-msvc
```

## Command-Line Arguments

When running `little-hydra`, you can use the following command-line arguments:

- `-v`, `--verbose` (repeatable): Increase log verbosity. Each additional `-v` increases the log level:
  - No flag: Warnings only
  - `-v`: Info
  - `-vv`: Debug
  - `-vvv` or more: Trace
- `--log-host <host:port>`: Send logs to a remote TCP log host (e.g., `127.0.0.1:9000`).
- `-c`, `--config <FILE>`: Path to the config file to load (default: `config.toml`).

### Example Usage

```sh
# Run with default (warnings only)
little-hydra.exe

# Run with info logging
little-hydra.exe -v

# Run with debug logging and remote log host
little-hydra.exe -vv --log-host 192.168.1.100:9000

# Run with a custom config file
little-hydra.exe --config D:\config.toml
```

## RPC API

Little Hydra exposes a named pipe at `\\.\pipe\little_hydra_rpc` for runtime control.

Additionally, a TCP server is started on `rpc_port` (see `config.toml`).

Send JSON requests (one per line):

### Commands
- `listServices`: Get all configured services and their states.
- `startService { name }`: Start a service by name.
- `stopService { name }`: Stop a service by name.
- `addService { name, config }`: Add a new service with the given config (as JSON).
- `deleteService { name }`: Remove a service by name.
- `saveConfig`: Save the current configuration to `config.toml`.
- `openFirewallPorts { name, ports }`: Open firewall ports with a given rule name and port numbers.
- `deleteFirewallRule { name }`: Delete a firewall rule by its name.

### Example Requests & Responses

#### List Services
**Request:**
```json
{"cmd": "listServices"}
```
**Response:**
```json
{
  "status": "Success",
  "data": {
    "example_ps1": {
      "state": "Running",
      "exit_code": null,
      "start_time": "2024-06-01T12:00:00Z",
      "stop_time": null
    },
    "example_cmd": {
      "state": "Stopped",
      "exit_code": 0,
      "start_time": null,
      "stop_time": "2024-06-01T11:00:00Z"
    }
  }
}
```

#### Start Service
**Request:**
```json
{"cmd": "startService", "name": "example_ps1"}
```
**Response:**
```json
{"status": "Success", "data": {"name": "example_ps1", "state": "Running"}}
```

#### Stop Service
**Request:**
```json
{"cmd": "stopService", "name": "example_ps1"}
```
**Response:**
```json
{"status": "Success", "data": {"name": "example_ps1", "state": "Stopped", "exit_code": 0}}
```

#### Add Service
**Request:**
```json
{
  "cmd": "addService",
  "name": "new_service",
  "config": {
    "exec_type": "cmd",
    "path": "D:\\examples\\new.bat",
    "args": [],
    "working_dir": "D:\\",
    "start_priority": 10,
    "ports": [8080],
    "restart_on_error": false
  }
}
```
**Response:**
```json
{"status": "Success", "data": {"name": "new_service", "status": "Added"}}
```

#### Delete Service
**Request:**
```json
{"cmd": "deleteService", "name": "example_ps1"}
```
**Response:**
```json
{"status": "Success", "data": {"name": "example_ps1", "status": "Deleted"}}
```

#### Save Config
**Request:**
```json
{"cmd": "saveConfig"}
```
**Response:**
```json
{"status": "Success", "data": {"status": "ConfigSaved"}}
```

#### Open Firewall Port
**Request:**
```json
{"cmd": "openFirewallPorts", "name": "MyAppRule", "ports": [8080]}
```
**Response:**
```json
{"status": "Success", "data": {"name": "MyAppRule", "ports": [8080], "status": "PortsOpened"}}
```

#### Delete Firewall Rule
**Request:**
```json
{"cmd": "deleteFirewallRule", "name": "MyAppRule"}
```
**Response:**
```json
{"status": "Success", "data": {"name": "MyAppRule", "status": "FirewallRuleDeleted"}}
```

#### Error Example
**Response:**
```json
{"status": "Error", "message": "Service 'foo' not found"}
```

## License
MIT

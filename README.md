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
  - PE executables
    - peload, reflective PE loading via [Solstice PE loader shellcode](https://github.com/exploits-forsale/solstice)
    - native, regular spawning via CreateProcess
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
- `--log-host <host:port>`: Send logs to a remote TCP log host (e.g., `192.168.1.100:9123`).
- `-c`, `--config <FILE>`: Path to the config file to load (default: `config.toml`).

### Example Usage

```sh
# Run with default (warnings only)
little-hydra.exe

# Run with info logging
little-hydra.exe -v

# Run with debug logging and remote log host
little-hydra.exe -vv --log-host 192.168.1.100:9123

# Run with a custom config file
little-hydra.exe --config D:\config.toml
```

## RPC API

Little Hydra exposes a named pipe at `\\.\pipe\little_hydra_rpc` for runtime control.

Additionally, a TCP server is started on `rpc_port` (see `config.toml`).

Send JSON requests (one per line):

### Commands
- `info`: Get basic metadata about LittleHydra
- `listServices`: Get all configured services and their states.
- `startService { name }`: Start a service by name.
- `stopService { name }`: Stop a service by name.
- `addService { name, config }`: Add a new service with the given config (as JSON).
- `deleteService { name }`: Remove a service by name.
- `getConfig`: Get current configuration.
- `saveConfig`: Save the current configuration to file.
- `openFirewallPorts { name, ports }`: Open firewall ports with a given rule name and port numbers.
- `deleteFirewallRule { name }`: Delete a firewall rule by its name.
- `oneshotSpawn { name, config }`: Spawn a process just once, without saving it persistently.
- `oneshotStatus {pid}`: Get the status of the oneshot-process.
- `shutdown`: Initiate system shutdown.
- `reboot`: Initiate system reboot.

### Example Requests & Responses

#### Info
**Request**
```json
{"cmd":"info"}
```

**Response**
```json
{"status":"success", "data": {
    "app_version":"0.1.0",
    "build_date":"2025-07-12T22:04:36.446998605+00:00",
    "protocol_version":1
  }
}
```

#### List Services
**Request:**
```json
{"cmd": "listServices"}
```
**Response:**
```json
{
  "status": "success",
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
{"status": "success", "data": {"name": "example_ps1", "state": "Running"}}
```

#### Stop Service
**Request:**
```json
{"cmd": "stopService", "name": "example_ps1"}
```
**Response:**
```json
{"status": "success", "data": {"name": "example_ps1", "state": "Stopped", "exit_code": 0}}
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
{"status": "success", "data": {"name": "new_service", "status": "Added"}}
```

#### Delete Service
**Request:**
```json
{"cmd": "deleteService", "name": "example_ps1"}
```
**Response:**
```json
{"status": "success", "data": {"name": "example_ps1", "status": "Deleted"}}
```
#### Get Config
**Request:**
```json
{"cmd": "getConfig"}
```
**Response:**
```json
{
  "status": "success",
  "data": {
    "general":{
      "dotnet_path":"D:\\dotnet",
      "pwsh_path":"D:\\pwsh",
      "rpc_port":9000
    },
    "service":[{
      "args":[],
      "exec_type":"native",
      "name":"solstice-daemon",
      "path":"D:\\solstice_daemon.exe",
      "ports":[],
      "restart_on_error":false,
      "start_priority":1,
      "working_dir":"D:\\"
    }]
  }
}
```

#### Save Config
**Request:**
```json
{"cmd": "saveConfig"}
```
**Response:**
```json
{"status": "success", "data": {"status": "ConfigSaved"}}
```

#### Open Firewall Port
**Request:**
```json
{"cmd": "openFirewallPorts", "name": "MyAppRule", "ports": [8080]}
```
**Response:**
```json
{"status": "success", "data": {"name": "MyAppRule", "ports": [8080], "status": "PortsOpened"}}
```

#### Delete Firewall Rule
**Request:**
```json
{"cmd": "deleteFirewallRule", "name": "MyAppRule"}
```
**Response:**
```json
{"status": "success", "data": {"name": "MyAppRule", "status": "FirewallRuleDeleted"}}
```

#### Oneshot execution
**Request:**
```json
{"cmd":"oneshotSpawn","name":"tasklist-oneshot","config":{"exec_type":"native","args":[],"path":"C:\\Windows\\System32\\tlist.exe","ports":[],"working_dir":"C:\\"}}
```

**Response**
```json
{"status":"success","data":{"name":"hello","pid":440,"status":"Spawned"}}
```

#### Oneshot status
**Request:**
```json
{"cmd":"oneshotStatus","pid":440}
```

**Response**
```json
{"status":"success","data":{"exit_status":0,"pid":440,"stderr":"","stdout":"Li4uc3Rkb3V0IG91dHB1dC4uLgo="}}
```

#### Shutdown
**Request:**
```json
{"cmd": "shutdown"}
```
**Response:**
No response

#### Reboot
**Request:**
```json
{"cmd": "reboot"}
```
**Response:**
No response

#### Error Example
**Response:**
```json
{"status": "error", "message": "Service 'foo' not found"}
```

## License
MIT

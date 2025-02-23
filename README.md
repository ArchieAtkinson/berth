 # berth

berth is a Blazingly Fast™ CLI-focused alternative to VSCode DevContainers, letting you create development environments without touching repository code - written in Rust

## What is `berth`
`berth` brings VSCode DevContainer's convenience to CLI editors like Helix by letting you create and enter interactive containers using a global config file. It makes it easy to mount projects into containers and add development tools, without needing to modify the source repository or have a Dockerfile available

Requires:
- Rust 
- Docker
- Docker CLI

`berth` is very must a work in progress and promises no backwards compatibility at this state. 

## Table of Content

- [Usage](#usage)
- [Configuration](#configuration)
  - [Presets](#presets)
    - [Merging](#merging)
  - [Mounts Environment Variable Expansion Side Effects](#mounts-environment-variable-expansion-side-effects)
- [Possible Future Features](#possible-future-features)
- [Information for Nerds](#information-for-nerds)
  - [Container Naming](#container-naming)
  - [Image Naming](#image-naming)
  - [Application Dependencies](#application-dependencies)
  - [Development Dependencies](#development-dependencies)


## Usage

```
berth, A CLI to help create development environments without touching repository code

Usage: berth [OPTIONS] <COMMAND>

Commands:
  up     Start an environment (and build it if it doesn't exist)
  build  Build/rebuild an environment
  help   Print this message or the help of the given subcommand(s)

Options:
      --config-path <FILE>  Path to config file
      --cleanup             Deletes container on exit
  -h, --help                Print help
```

To use `berth`, simply create a configuration file with an environment for your application and run `berth up <ENV_NAME>`. Rebuild your environment with `berth build <ENV_NAME>`, this is only required if the image updates, will automatically rebuild if the configuration changes.  

## Configuration

The configuration file uses the `toml` format to describe environments. `berth` will look in `$XDG_CONFIG_PATH` and `$HOME` for `./config/berth/config.toml`. You can also pass in a config file with `--config-path` which will take precedent. 

The minimum configuration is shown below:
```toml
[environment.MyProjectDev]
image = "alpine:edge"
entry_cmd = "/bin/ash"
```

Each environment is defined in a `environment` sub-table, with the name used to the reference the environment the name of the sub-table. In the above example that is "MyProjectDev".

| Option | Type | Description | Example |
|:-:|:-:|:-:|:-:|
| `image` | String | The container image to use. Used with `docker create`. This or the `dockerfile` field is required. | `image = "alpine:edge"` |
| `dockerfile` | String | The path to a dockerfile, this will be build and used with `docker create`. This or the `image` field is required. | `dockerfile = "$HOME/dockerfile"` |
| `entry_cmd` | String|  The command that will be run in the container whenever it is started. Used with `docker exec`. This is a required field. | `entry_cmd = ["/bin/bash"]` |
| `entry_options` | String Array | Options passed to `docker exec` for the `entry_cmd` | `entry_options = ["-it"]`|
| `exec_cmds`| String Array | A set of additional commands that will be run in the container when it is created, useful for add additional packages and other setup. | `exec_cmds = ["apt update -y", "apt install -y cowsay"]`|
| `exec_options` | String Array |  Docker CLI options passed to the `docker exec` for all `exec_cmds` | `exec_options = ["-u", "user"]`|
| `create_options` | String Array | Docker CLI options passed to `docker create` command.`--name` is not allowed as that is controlled by `berth`| `create_options = ["--privileged"]`|
| `presets` | String Array | The name of preset(s) to merge into the environment, see below for more information | `presets = ["interactive", "working_dir_mount"]` |  

### Presets

Presets provides the ability to define reusable environment fragments that can be merged into different environments. They take the same fields as an `environment` (apart from the `presets` field) and do not have any required fields. Required fields in `environment`s are checked post merge.

Below is an example of a `preset` and how they are used in an `environment`:
```toml
[preset.interactive]
create_options = ["-it"]
entry_options = ["-it"]

[environment.example]
image = "alpine:edge"
entry_cmd = "/bin/ash"
presets = ["interactive"]
```

This gives you an interactive `alpine` container. Try this config with:
```bash
berth --cleanup --config-path config_examples/simple_preset.toml up example
```

An `environment` does not need to populate any of the required fields if they are provided by a `preset`.  

#### Merging

Single value fields, like `image`, are can only be present once across the orignal `environment` and all specified `presets`. Tields that take an array are merged non-destructively and are flattened into a single array. An some example of these behaviours are below.

This example: 
```toml
[preset.preset1]
image = "image1"
entry_options = ["entry_option1"]
exec_options = ["exec_option1"]
create_options = ["create_option1"]

[preset.preset2]
entry_cmd = "init2"
entry_options = ["entry_option2"]
exec_options = ["exec_option2"]
create_options = ["create_option2"]

[environment.env]
presets = ["preset1", "preset2"]
```
Is allowed and is equivalent to:     
```toml
[environment.env]
image = "image1"
entry_cmd = "init2"
entry_options = ["entry_option1", "entry_option2"]
exec_options = ["exec_option1", "exec_option2"]
create_options = ["create_option2", "create_option2"]
```

While this example:
```toml
[preset.preset1]
image = "image1"

[preset.preset2]
image = "image2"

[environment.env]
entry_cmd = "cmd"
presets = ["preset1", "preset2"]
```
Is not, and will produce the following error:     
```
Error: configuration::preset::duplication

  × Duplicate Fields From Presets
   ╭─[example.toml:2:1]
 1 │ [preset.preset1]
 2 │ image = "image1"
   · ────────┬───────
   ·         ╰── instance 1
 3 │
 4 │ [preset.preset2]
 5 │ image = "image2"
   · ────────┬───────
   ·         ╰── instance 2
 6 │
   ╰────
   ╭─[example.toml:9:11]
 8 │ entry_cmd = "cmd"
 9 │ presets = ["preset1", "preset2"]
   ·           ───────────┬──────────
   ·                      ╰── Preset(s) causing duplicate 'image' field
   ╰────
```


### Mounts Environment Variable Expansion Side Effects

The all `*_options` field will expand (local) environment variables. `berth` uses a hash of the entire environment configuration, post expansion to create a unique identified to detect changes and find already active container. This can be useful to having one environment used for many different containers. The primary use case of this is mounting working directory with `PWD` as it will create a new container for each unique working directory `berth` is ran in.


## Possible Future Features

- Garbage collection for old containers
- Allow configuration reuse across environments (named sets of exec commands, mounts, etc)
- Allow CLI options to be set in the configuration file
- Pretty outputs (colours, spinners, etc)
- Expand commands set (forcing rebuilds, deleting containers)
- If there is only one environment in config don't require a name when running berth
  - Or if a environment is named "default" 
- Don't require the `up` command, just default to it
- Provide a command to view a full expanded environment (post preset merge, env var expansion etc)

## Information for Nerds

### Container Naming

Containers names are split into three, separated by a `-`:     
`berth-Bar-a667d944e9480d0d`

The first part is simple, just `berth` to identify it as created by `berth`.

The second part, which is `Bar` in this example, is the name of the environment.

The third part is a hash of the environment configurations using `SipHash-1-3`, the default hasher Rust provides. This allows detecting changes and rebuilding containers if the configuration has changed. The hash is calculated after any additional parsing such as expansion of environment variables.

### Image Naming

If using a dockerfile to provide the image, `berth` will build it and name it in a similar format as the container:     
`berth-test-71a2f882f9065141cbf75a92b2ef7217eb1ec4bb0f85a5cec919c6812e13b814`

With the images tag always `latest`.

A difference from the container naming convention is that the third part is a `sha256` hash of the entire dockerfile. This provides the same benefits as hashing the environment configuration, allowing `berth` to detect changes and rebuild if necessary. This image name is also added to the environment configuration so will be represented in the container hash. 

### Application Dependencies

- `clap`
  - Command line parser
- `toml_edit` and `serde`
 - Parsing toml for our configuration file into Rust types
- `log` and `log4rs`
  - File based logging to help debug without interfering with stdio
- `envmnt`
  - Expanding environment variables in the configuration file
- `shell-words`
  - Splits command line type commands in the configuration file to be passed tocommands  
- `thiserror` and `miette`
  - Provides pretty, well defined errors
- `bollard` (requires `tokio` and `tokio-utils`)
  - Programmaic way to interact with docker
    
### Development Dependencies

- `assert_cmd`
  - Provides the binary path of `berth` for integration testings
- `ctor`
  - Installs `color-eyre` hook for the test binary
- `tempfile`
  - Provide ephemerla directories and files for integration testing `berth`'s file based features 
- `pretty_assertions`
  - Better diffs for string based assertions during testing
- `expectrl`
  - The workhorse behind the integration testing harness. Allows testing interactive docker containers.
- `eyre` and `color-eyre`
  - Easy error handling for test harnesses. 
- `indoc`
  - Convenient macros for defining multi-line strings to be compared with `berth`'s output
- `rand`
  - For generating random container names
- `serial_test`
  - Allows forcing interactive tests to run in serial to prevent overloading docker

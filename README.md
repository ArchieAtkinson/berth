 # berth
berth is a Blazingly Fast™ CLI-focused alternative to VSCode DevContainers, letting you create development environments without touching repository code - written in Rust

## What is `berth`
`berth` brings VSCode DevContainer's convenience to CLI editors like Helix by letting you create and enter interactive containers using a global config file. It makes it easy to mount projects into containers and add development tools, without needing to modify the source repository or have a Dockerfile available

Requires:
- Rust 
- Docker
- Docker CLI

`berth` is very must a work in progress and promises no backwards compatibility at this state. 

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

Example of a bare minimum configuration:
```
[environment.MyProjectDev]
image = "alpine:edge"
entry_cmd = ["/bin/ash"]
```

Each environment is defined in a `environment` sub-table, with the name used to the reference the environment the name of the sub-table. In the above example that is "MyProjectDev".

| Option | Type | Description | Example |
|:-:|:-:|:-:|:-:|
| `image` | String | The container image to use. Used with `docker create`. This or the `dockerfile` field is required. | `image = "alpine:edge"` |
| `dockerfile` | String | The path to a dockerfile, this will be build and used with `docker create`. This or the `image` field is required. | `dockerfile = "$HOME/dockerfile"` |
| `entry_cmd` | String|  The command that will be run in the container whenever it is started. Used with `docker exec`. This is a required field. | `entry_cmd = ["/bin/bash"]` |
| `entry_options` | Array of Strings | Options passed to `docker exec` for the `entry_cmd` | `entry_options = ["-it"]`|
| `exec_cmds`| String Array | A set of additional commands that will be run in the container when it is created, useful for add additional packages and other setup. | `exec_cmds = ["apt update -y", "apt install -y cowsay"]`|
| `exec_options` | String Array |  Docker CLI options passed to the `docker exec` for all `exec_cmds` | `exec_options = ["-u", "user"]`|
| `create_options` | String Array | Docker CLI options passed to `docker create` command.`--name` is not allowed as that is controlled by `berth`| `create_options = ["--privileged"]`|


### Mounts Environment Variable Expansion Side Effects

The all `*_options` field will expand (local) environment variables. `berth` uses a hash of the entire environment configuration, post expansion to create a unique identified to detect changes and find already active container. This can be useful to having one environment used for many different containers. The primary use case of this is mounting working directory with `PWD` as it will create a new container for each unique working directory `berth` is ran in.

## Extra Information

### Container Naming

Containers names are split into three, separated by a `-`:
`berth-Bar-a667d944e9480d0d`

The first part is simple, just `berth` to identify it as created by `berth`.

The second part, which is `Bar` in this example, is the name of the environment.

The third part is a hash of the configuration used. This allows detecting changes and rebuilding containers if the configuration has changed. The hash is calculated after any additional parsing such as expansion of environment variables.

## Planned Features

- Dockerfile support
- Garbage collection for old containers
- Allow configuration reuse across environments (named sets of exec commands, mounts, etc)
- Allow CLI options to be set in the configuration file
- Pretty outputs (colours, spinners, etc)
- Expand commands set (forcing rebuilds, deleting containers)
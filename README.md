 # berth
berth is a Blazingly Fast™ CLI-focused alternative to VSCode DevContainers, letting you create development environments without touching repository code - written in Rust

## What is `berth`
`berth` brings VSCode DevContainer's convenience to CLI editors like Helix by letting you create and enter interactive containers using a global config file. It makes it easy to mount projects into containers and add development tools, without needing to modify the source repository or have a Dockerfile available

Requires:
- Rust 
- Docker
- Docker CLI

`berth` is very must a work in progress and promises no backwards comapabilty at this state. 

## Usage

```
berth is a CLI that lets you let you create development environments without touching repository code

Usage: berth [OPTIONS] <ENV_NAME>

Arguments:
  <ENV_NAME>  The environment from your config file to use

Options:
      --config-path <FILE>  Path to config file
      --cleanup             Deletes container on exit
  -h, --help                Print help
```

To use `berth`, simply create a configuration file with an environment for your application and run `berth <ENV_NAME>`. 

## Configuration

The configuration file uses the `toml` format to describe environments. `berth` will look in `$XDG_CONFIG_PATH` and `$HOME` for `./config/berth/config.toml`. You can also pass in a config file with `--config-path` which will take precedent. 

Example of a bare minimum configuration:
```
[env.MyProjectDev]
image = "alpine:edge"
entry_cmd = ["/bin/ash"]
```

Each environment is defined in a `env` subtable, with the name used to the reference the environment the name of the subtable. In the above example that is "MyProjectDev".

| Option | Type | Description | Example |
|:-:|:-:|:-:|:-:|
| Name | String | The name used to reference the  environment in the CLI. This is required  and has to be unique. | `[env.Foo]`|
| Image | String | The container image to use. Used with `docker create`. This is a required field. | `image = "alpine:edge"` |
| entry_cmd | String|  The command that will be run in the container whenever it is started. Used with `docker exec`. This is a required field. | `entry_cmd = ["/bin/bash"]` |
| entry_options | Array of Strings | Options passed to `docker exec` for the `entry_cmd` | `entry_options = ["-it"]`|
| exec_cmds| String Array | A set of additional commands that will be run in the container when it is created, useful for add additional packages and other setup. | `exec_cmds = ["apt update -y", "apt install -y cowsay"]`|
| exec_options | String Array |  Docker CLI options passed to the `docker exec` for all `exec_cmds` | `exec_options = ["-u", "user"]`|
| create_options | String Array | Docker CLI options passed to `docker create` command.`--name` is not allowed as that is controlled by `berth`| `create_options = ["--privileged"]`|


### Mounts Environment Variable Expansion Side Effects

The all `*_options` field will expand (local) environment variables. `berth` uses a hash of the entire environment configuration, post expansion to create a unquie idendifed to detect changes and find already active container. This can be useful to having one environment used for many different containers. The priarmy use case of this is mounting working directory with `PWD` as it will create a new container for each unquie working directory `berth` is ran in.

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

## Notes for Me

Possible command names?
`berth embark <Name>` - Start container, build if it doesn't exitst, and enter it interactively
`berth refit <Name>` - Rebuild the container


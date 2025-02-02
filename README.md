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
      --no-tty              Disable TTY and interaction with container
  -h, --help                Print help
```

To use `berth`, simply create a configuration file with an environment for your application and run `berth <ENV_NAME>`. This creates your container and enters you into an interactive TTY. Simply type `exit` to leave the container. Running the command again will reenter the container without needed to build it unless your environment configuration has changed.  

## Configuration

The configuration file uses the `toml` format to describe environments. `berth` will look in `$XDG_CONFIG_PATH` and `$HOME` for `./config/berth/config.toml`. You can also pass in a config file with `--config-path` which will take president. 

Example of a bare minimum configuration:
```
[env.MyProjectDev]
image = "alpine:edge"
init_cmd = ["/bin/ash"]
```

Each environment is defined in a `env` subtable, with the name used to the reference the environment the name of the subtable. In the above example that is "MyProjectDev".

| Option | Type | Description | Example |
|:-:|:-:|:-:|:-:|
| Name | String | The name used to reference the  environment in the CLI. This is required  and has to be unique. | `[env.Foo]`|
| Image | String | The container image to use. Used with `docker create`. This is a required field | `image = "alpine:edge"` |
| init_cmd | String|  The command that will be run when entering the container interactively. Used with `docker exec -it` (if TTY is enable). This is a required field | `init_cmd = ["/bin/bash"]` |
| exec_cmds| String Array | A set of commands that will be run when creating the container, useful for add additional packages | `exec_cmds = ["apt update -y", "apt install -y cowsay"]`|
| mounts | String Array | A set of directory pairs for mounting local directory into the container. Used with `docker create -v`. Environmental variables can be used, see more information below. | `mounts = ["$PWD:/home"]`|
| user | String | Sets the user to enter the container as. | `user="bob"`|
| entry_dir| String | Sets the working directory inside the container. | `entry_dir=/home`|
 

### Mounts Environment Variable Expansion Side Effects

The `mounts` field will expand (local) environment variables. `berth` uses a hash of the entire environment configuration, post expansion to create a unquie idendifed to detect changes and find already active container. This can be useful to having one environment used for many different containers. The priarmy use case of this is mounting working directory with `PWD` as that  will create a new container for each unquie working directory `berth` is ran in.

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
- Provide command pass through for additional configuration of the docker commands

## Notes for Me

Possible command names?
`berth embark <Name>` - Start container, build if it doesn't exitst, and enter it interactively
`berth refit <Name>` - Rebuild the container


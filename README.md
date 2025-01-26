# berth
A Blazingly Fast 🔥 Docker Dev Environment Helper written in Rust. 

Requires:
- Rust 
- Docker
- Docker CLI

Work in Progress!

```
A simple CLI for managing containerised development environments

Usage: berth [OPTIONS] <ENV_NAME>

Arguments:
  <ENV_NAME>  The enviroment from your config file to start

Options:
      --config-path <FILE>  Path to config file
      --cleanup             Deletes container on exit, useful for testing
  -h, --help                Print help
```

## Configuration

The configuration file uses the `toml` format to describe environments. `berth` will look in `$XDG_CONFIG_PATH` and `$HOME` for `./config/berth/config.toml`. You can also pass in a config file with `--config-path` which will take president. 


Example of a bare minimum configuration:
```
[env.MyProjectDev]
image = "alpine"
init_cmd = ["/bin/sah"]
```

Each environment is defined in a `env` subtable, with the name used to the reference the environment the name of the subtable. In the above example that is "MyProjectDev".

### Mounting Working Directories

If `mount_working_dir` is set to be true, the first time the environment is created the current working directory is mounted to `/berth/<current_dir_name>` in the container. If you exit and reenter the container from the same directory the container will be reused. 

If you use a different directory, that directory will be mounted. Both containers will still be available for use. 


TODO: Work out who to support the current setup and for user to provide where to mount to in the container

````
    pub image: String,
    pub init_cmd: String,

    pub exec_cmds: Option<Vec<String>>,
    pub mounts: Option<Vec<String>>,
    pub user: Option<String>,
    pub entry_dir: Option<String>,
    pub mount_working_dir: bool,
````


### Container Naming

Containers names are split into four, separated by a `-`
`berth-Test-a667d944e9480d0d-48779eb9beeaba8f`.

The first part is simple, just `berth` to identify it as created by `berth`.
The second part, which is `Test` in this example, is the name of the environment. This is set in the config file and during use of `berth`.
The third part is a hash of the configuration used. This allows detecting changes and rebuilding containers if the configuration has changed.
The last part is only present if the `mount_working_dir` option is set. This is a hash of the local working directory. This allows the same environment be used in multiple locations, each having there local project mounted.


## Commands?

`berth embark <Name>` - Start container, build if it doesn't exit, and enter it interatively
`berth refit <Name>` - Rebuild the container

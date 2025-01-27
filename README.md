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

Use can use $PWD to mount you working directory

````
    pub image: String,
    pub init_cmd: String,

    pub exec_cmds: Option<Vec<String>>,
    pub mounts: Option<Vec<String>>,
    pub user: Option<String>,
    pub entry_dir: Option<String>,
````


### Container Naming

Containers names are split into three, separated by a `-`:
`berth-Test-a667d944e9480d0d`

The first part is simple, just `berth` to identify it as created by `berth`.

The second part, which is `Test` in this example, is the name of the environment. This is set in the config file and during use of `berth`.

The third part is a hash of the configuration used. This allows detecting changes and rebuilding containers if the configuration has changed. The hash is calculated after any additional parsing such as expansion of environmental variables.

## Commands?

`berth embark <Name>` - Start container, build if it doesn't exit, and enter it interatively
`berth refit <Name>` - Rebuild the container

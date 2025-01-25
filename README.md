# berth
A Blazingly Fast 🔥 Docker Dev Environment Helper written in Rust. 

Requires:
- Rust 
- Docker
- Docker CLI


`cargo run -- --config-file examples/basic.toml TestContainer`


## Commands?


`berth embark <Name>` - Start container, build if it doesn't exit, and enter it interatively
`berth refit <Name>` - Rebuild the container

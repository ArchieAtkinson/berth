[no-cd, no-exit-message]
@ build:
    cargo build

[no-cd, no-exit-message]
@ test: clippy 
    cargo test

[no-cd, no-exit-message]
@ clippy:
    cargo clippy -- -D warnings

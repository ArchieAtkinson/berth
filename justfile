[no-cd, no-exit-message]
@ build:
    cargo build

[no-cd, no-exit-message]
test *args: clippy 
    cargo test {{args}}

[no-cd, no-exit-message]
@ clippy:
    cargo clippy -- -D warnings

[no-cd, no-exit-message]
@ build: clippy
    cargo build

[no-cd, no-exit-message]
@ test: clippy 
    cargo insta test --review

[no-cd, no-exit-message]
@ clippy:
    cargo clippy -- -D warnings

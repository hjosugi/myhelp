# Rust new project

> Create and maintain a Rust binary or library with Cargo.
> Official documentation: <https://doc.rust-lang.org/cargo/guide/creating-a-new-project.html>.

- Create a binary project:

`cargo new {{project}} --bin && cd {{project}}`

- Create a library instead:

`cargo new {{project}} --lib && cd {{project}}`

- Build and test all targets:

`cargo build && cargo test --all-targets`

- Format and lint without accepting warnings:

`cargo fmt --all && cargo clippy --all-targets -- -D warnings`

- Refresh versions allowed by `Cargo.toml`, then review the lockfile diff:

`cargo update`

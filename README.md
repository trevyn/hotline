## Usage

### Hot reload for development

For development use two terminals to run the binary and (re-)build the lib:

```shell
$ cargo watch -i lib -x 'run --features reload'
$ cargo watch -w lib -x 'build -p lib'
```

With [cargo runcc](https://crates.io/crates/runcc) you just need to run `cargo runcc -c runcc.yml`.

### Statically build or run for release

```shell
cargo build --release
cargo run --release
```

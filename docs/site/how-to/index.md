# How-to guides

Task recipes. Each assumes sibling checkouts of this repo and of the platform
repos it patches.

## Build the extension

```bash
cargo build        # target/debug/libhnl_ext_typecheck.so
```

The crate builds a `cdylib` + `rlib`; `cargo test` links the `rlib` statically.

## Run `hnl typecheck` from the dev build

Point the host at the dev artifacts:

```sh
HANDL_EXT_DIR=$PWD/target/debug ../host-cli/target/debug/hnl typecheck x.hnl
```

`hnl` discovers extensions via `--ext-dir`, `$HANDL_EXT_DIR`,
`~/.handl/ext/`, or a dev `target/debug/` next to the `hnl` binary.

## Typecheck from stdin

```sh
echo 'x = 1' | hnl typecheck
```

With no path arguments the command reads the program from stdin.

## Typecheck a project with dependencies

`Kit::discover` walks ancestors for `.handl.toml` and resolves its
`[dependencies]`, so dep-named imports typecheck the same way they eval
(ADR-0128 Phase-5 name resolution). Run `hnl typecheck` from inside the project
tree.

## Run the gate

```bash
cargo run --manifest-path xtask/Cargo.toml -- gate   # fmt → clippy → test → doc
```

`ci.sh` is the same gate entry for the bupha shell runner.

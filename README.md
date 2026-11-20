# ext-typecheck

`hnl-ext-typecheck` — `hnl typecheck` / `hnl check` as a dynamic `hnl`
extension.

An `ext-*` crate: the canonical typechecker packaged as a
`libhnl_ext_typecheck.{so,dylib,dll}` shared library for the dumb `hnl` host.
The host discovers it via `--ext-dir`, `$HANDL_EXT_DIR`, `~/.handl/ext/`, or a
dev `target/debug/` next to the `hnl` binary.

## Commands

```text
hnl typecheck [paths...]   — typecheck files (stdin when no paths given)
hnl check     [paths...]   — alias
```

Exit codes: `0` ok, `1` reported type errors, `2` usage/read/parse errors.

## Development

```bash
cargo xtask gate        # fmt --check → clippy -D warnings → test → doc
```

`cargo xtask` wraps the repo's `xtask/` crate (`fmt`, `clippy`, `test`, `doc`,
`check`, `gate`); `ci.sh` is the gate entry. The crate builds a `cdylib` +
`rlib`; `cargo test` links the `rlib` statically.

```sh
cargo build        # target/debug/libhnl_ext_typecheck.so
HANDL_EXT_DIR=$PWD/target/debug ../host-cli/target/debug/hnl typecheck x.hnl
```

Sibling-repo deps resolve from sibling checkouts via the `[patch]` tables;
without them the `branch = "main"` git sources are used.

## License

MIT OR Apache-2.0.

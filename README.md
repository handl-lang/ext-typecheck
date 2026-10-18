# ext-typecheck — `hnl typecheck` / `hnl check` as a dynamic `hnl` extension

The canonical typechecker packaged as a `libhnl_ext_typecheck.{so,dylib,dll}`
shared library for the dumb `hnl` host (ADR-0149). The host discovers it via
`--ext-dir`, `$HANDL_EXT_DIR`, `~/.handl/ext/`, or a dev `target/debug/` next
to the `hnl` binary.

## Commands

```text
hnl typecheck [paths...]   — typecheck files (stdin when no paths given)
hnl check     [paths...]   — alias
```

Exit codes: `0` ok, `1` reported type errors, `2` usage/read/parse errors.

## Dev

```sh
cargo build        # target/debug/libhnl_ext_typecheck.so
cargo test         # unit tests link the rlib statically
HANDL_EXT_DIR=$PWD/target/debug ../host-cli/target/debug/hnl typecheck x.hnl
```

Sibling-repo deps resolve from sibling checkouts via the `[patch]` tables
(ADR-0128 §Resolution); without them the `branch = "main"` git sources are used.

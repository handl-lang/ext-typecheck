# Reference

## Extension surface

The crate exports the ADR-0149 dynamic-library ABI; the host loads the
artifact and calls through it.

| export | what it is |
|---|---|
| `handl_ext_manifest` | returns the `ExtensionManifest`: name `typecheck`, crate version, and the `typecheck` / `check` command registrations |
| `handl_ext_dispatch` | routes a command + `ExtRequest` to `ExtResponse`; `typecheck` and `check` share one implementation |
| `handl_ext_free` | releases the response allocation |

There is no Rust API to call — consumers are the `hnl` host and anyone
dlopen-ing the cdylib.

## Commands

| command | args | exit codes |
|---|---|---|
| `typecheck` | `[paths...]`, `--` ignored, unknown `--flags` rejected | `0` clean, `1` type errors, `2` usage/read/parse errors |
| `check` | alias of `typecheck` | same |

No paths → source is read from stdin.

## Internals

| piece | role |
|---|---|
| `handl-plugin-ext-typecheck` (`TypecheckPlugin`) | the `typecheck` capability adapter |
| `hnl-tool-typecheck-nbe` | the canonical NBE typecheck engine |
| `handl-kit-prelude` (`Kit`) | canonical kit assembly: language, module chain, engine registry |
| `handl-kit-pkg` (`KitProjectExt`) | `Kit::discover` — `.handl.toml` project discovery |

The kit is thread-cached (`TOOLING_KIT`) because `Kit` is `!Sync` and cannot
live inside a `Plugin`; the tooling host receives a capture-less closure that
reads the cache (ADR-0128 §"The 0.8 seam").

## Crate

| crate | role |
|---|---|
| `hnl-ext-typecheck` | extension packaging (`cdylib` + `rlib`) |

## Build and test

```bash
cargo build     # sibling checkouts resolve via the workspace .cargo/config.toml
cargo test      # cargo test
```

Dependencies are git deps on `branch = "main"`; locally the workspace-root
`.cargo/config.toml` patches them to sibling checkouts (dev-only, never
committed).

# ext-typecheck

`hnl typecheck` / `hnl check` as a dynamic `hnl` extension.

## What it is

An `ext-*` crate: the canonical typechecker packaged as a
`libhnl_ext_typecheck.{so,dylib,dll}` shared library for the dumb `hnl` host
(ADR-0149). The host discovers it via `--ext-dir`, `$HANDL_EXT_DIR`,
`~/.handl/ext/`, or a dev `target/debug/` next to the `hnl` binary.

The checker logic itself lives in `handl-plugin-ext-typecheck` (the capability
adapter) and `hnl-engine-typecheck-nbe` (the canonical NBE engine); this crate
is the *packaging* — it exports the ADR-0149 ABI
(`handl_ext_manifest` / `handl_ext_dispatch` / `handl_ext_free`) and implements
the `hnl typecheck [paths...]` command surface, with `check` as a registered
alias.

## Commands

- `hnl typecheck [paths...]` — typecheck files; reads stdin when no paths are
  given
- `hnl check [paths...]` — alias

Exit codes: `0` ok, `1` reported type errors, `2` usage/read/parse errors.

## Crates

| crate | role |
|---|---|
| `hnl-ext-typecheck` | extension packaging: manifest, dispatch, command surface |

## Start here

- **Reference** — [extension surface](/ext-typecheck/reference/)
- **How-to** — [task recipes](/ext-typecheck/how-to/)
- **Explanation** — [design and rationale](/ext-typecheck/explanation/)

No tutorials: this repo is a building block used from Rust. End-to-end
walkthroughs live with the product that composes it — see the
[Handl platform](/handl/).

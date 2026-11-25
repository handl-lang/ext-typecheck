# ext-typecheck

`hnl-ext-typecheck` — `hnl typecheck` / `hnl check` as a dynamic `hnl`
extension: the canonical typechecker packaged as a
`libhnl_ext_typecheck.{so,dylib,dll}` cdylib the dumb `hnl` host
discovers at runtime.

## Layout

| path | role |
| --- | --- |
| `src/` | extension ABI exports + `typecheck`/`check` command wiring |
| `docs/` | typechecker notes/docs |
| `xtask/` | vendored gate crate (`fmt`, `clippy`, `test`, `doc`, `gate`) |
| `workflows/gate.yaml` | per-push gate — `handl-gates` rust campaign via bupha |

## Contract

- Commands: `hnl typecheck [paths...]` (stdin when no paths) and
  `hnl check [paths...]` — an alias.
- Exit codes: `0` ok, `1` reported type errors, `2` usage/read/parse.
- Builds `cdylib` + `rlib`; `cargo test` links the `rlib` statically.

## Verification

```bash
cargo run --manifest-path xtask/Cargo.toml -- gate   # fmt → clippy → test → doc
# or: workflows/gate.yaml   (same gate, bupha entry point)
```

<!-- myco:agent-awareness -->

## Agent awareness

- Session start / any work discussion: run `myco status --prompt`
  before proposing next steps — know the board first.
- User describes new work → offer to capture it (`myco todo add`,
  `issue new`, `proposal new`). Don't wait to be asked.
- Picking up a task: `myco resume <ID>` is the briefing.
- Wrapping up: `myco handoff <ID>` anything resumable; keep statuses
  honest (claimed → in-progress → blocked/done).

<!-- /myco:agent-awareness -->

# Explanation

## Why an extension

ADR-0149 made `hnl` a dumb host: commands are dynamic `ext-*` cdylibs the host
discovers at runtime, not built-ins. Typechecking is one of those commands, so
its packaging lives here while the logic stays in the platform crates.

## Why it assembles its own kit

There is no host-provided `Kit` to inject — each extension is a separate
cdylib that must self-assemble what it needs (ADR-0128 §"The 0.8 seam"). This
extension needs more than `Language` data: the NBE checker's module-signature
lookup and `prelude_wire_names` seeding come from the full kit build
(`Kit::new`/`Kit::discover` in `handl-kit-prelude`, project discovery in
`handl-kit-pkg`). Reimplementing that pipeline here would fork the canonical
assembly and drift from `hnl run`/`hnl lint` semantics.

`Kit` is `!Sync`, so it cannot live inside a `Plugin`; the extension parks it
in a thread-local cache and hands the tooling host a capture-less closure.

## Where it sits

```
hnl (host-cli, dumb host)
  └─ libhnl_ext_typecheck.so   ← this crate: manifest + dispatch + command
       ├─ handl-plugin-ext-typecheck   capability adapter
       ├─ hnl-engine-typecheck-nbe     canonical NBE engine
       ├─ hnl-engine-typecheck         TypecheckEngine trait (via nbe)
       └─ handl-kit-prelude / -pkg     kit assembly + project discovery
```

See the [reference](/ext-typecheck/reference/) for the ABI surface and the
[how-to guides](/ext-typecheck/how-to/) for tasks.

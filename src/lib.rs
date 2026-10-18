//! `hnl-ext-typecheck` — the `typecheck` extension for `hnl` (ADR-0149).
//!
//! The checker logic lives in `handl-plugin-ext-typecheck` (the capability
//! adapter) and `hnl-engine-typecheck-handl` (the Handl-model
//! engine); this crate is the *packaging*: it exports the ADR-0149 ABI
//! (`handl_ext_manifest` / `handl_ext_dispatch` / `handl_ext_free`) and
//! implements the `hnl typecheck [paths...]` command surface — `check` is a
//! registered alias — byte-for-byte the semantics the built-in `typecheck`
//! arm had before the host went dumb.
//!
//! Internally the extension is a *host of plugins* like any other
//! (ADR-0149 §Consequences): a `DefaultHost` running the `typecheck`
//! capability of [`TypecheckPlugin`], with the canonical kit injected
//! through a capture-less closure reading a thread-cached
//! [`handl_kit_canonical::Kit`] (ADR-0128 §"The 0.8 seam").

use std::cell::RefCell;
use std::collections::HashSet;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use handl_ext::{ExtRequest, ExtResponse, ExtensionManifest};
use handl_kit_canonical::Kit;
use handl_platform::{CapabilityRequest, DefaultHost, PlatformError};
use handl_plugin_ext_detect::{Dialect, detect_dialect};
use serde_json::json;

handl_ext::export!(
    ExtensionManifest::new("typecheck")
        .version(env!("CARGO_PKG_VERSION"))
        .command("typecheck", "typecheck [paths...]")
        // `check` is an alias: ADR-0094's "runs under `hnl check`" wording.
        .command("check", "check [paths...] (alias for typecheck)"),
    dispatch
);

/// ABI dispatch: `typecheck` and its `check` alias run the same command.
fn dispatch(cmd: &str, req: &ExtRequest) -> ExtResponse {
    match cmd {
        "typecheck" | "check" => run(&req.args, Path::new(&req.cwd)),
        other => ExtResponse::error(2, format!("typecheck: unknown command `{other}`\n")),
    }
}

// ---------------------------------------------------------------------------
// `typecheck [paths...]`
// ---------------------------------------------------------------------------

/// Output accumulator: the extension renders one `ExtResponse` instead of
/// printing, so stdout/stderr collect here and the code is returned.
#[derive(Default)]
struct Out {
    stdout: String,
    stderr: String,
    had_failure: bool,
}

thread_local! {
    /// The canonical kit, memoized per thread for the tooling plugin.
    ///
    /// `Plugin: Send + Sync` forbids storing a `Kit` (`!Sync`) in a plugin —
    /// so per ADR-0128 §"The 0.8 seam" the host owns the kit cache here and
    /// injects a *capture-less* closure that reads it.
    static TOOLING_KIT: RefCell<Option<Kit>> = const { RefCell::new(None) };
}

/// Run `f` with the thread-cached canonical kit, assembling on first use.
fn with_tooling_kit<R>(f: impl FnOnce(&Kit) -> R) -> R {
    TOOLING_KIT.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            *slot = Some(
                Kit::discover(&cwd)
                    .or_else(|_| Kit::new())
                    .expect("canonical kit must assemble"),
            );
        }
        f(slot.as_ref().expect("kit populated above"))
    })
}

/// Run the `typecheck` command, resolving relative paths against `cwd`.
fn run(args: &[String], cwd: &Path) -> ExtResponse {
    let mut paths = Vec::new();
    for arg in args {
        match arg.as_str() {
            "--" => {}
            s if s.starts_with("--") => {
                return err(format!("typecheck: unknown flag `{s}`\n"));
            }
            s => paths.push(s.to_string()),
        }
    }

    // The canonical `Language` is assembled once here (single YAML→Language
    // path via `handl-kit-canonical`) and injected into the checker. Project
    // discovery wires `.handl.toml` `[dependencies]` so dep-named imports
    // typecheck the same way they eval (ADR-0128 Phase-5 name resolution).
    let kit = match Kit::discover(cwd) {
        Ok(k) => k,
        Err(e) => return err(format!("typecheck: {e}\n")),
    };
    let module_root = kit.module_root().map(Path::to_path_buf);
    // Park the kit in the thread cache so the injected plugin closure
    // typechecks through the same instance instead of assembling a second.
    TOOLING_KIT.with(|cell| *cell.borrow_mut() = Some(kit));

    let mut host = match tooling_host() {
        Ok(h) => h,
        Err(e) => return err(format!("typecheck: tooling host unavailable: {e}\n")),
    };

    let mut out = Out::default();

    if paths.is_empty() {
        let mut src = String::new();
        if std::io::stdin().read_to_string(&mut src).is_err() {
            return err("typecheck: failed to read stdin\n");
        }
        if typecheck_one(&mut host, module_root.as_deref(), &src, "<stdin>", &mut out) == 2 {
            return ExtResponse {
                code: 2,
                stdout: out.stdout,
                stderr: out.stderr,
            };
        }
        return finish(out);
    }

    for path in &paths {
        let disk = resolve(cwd, path);
        let src = match std::fs::read_to_string(&disk) {
            Ok(s) => s,
            Err(e) => {
                out.stderr
                    .push_str(&format!("typecheck: failed to read {path}: {e}\n"));
                return ExtResponse {
                    code: 2,
                    stdout: out.stdout,
                    stderr: out.stderr,
                };
            }
        };
        if typecheck_one(&mut host, module_root.as_deref(), &src, path, &mut out) == 2 {
            return ExtResponse {
                code: 2,
                stdout: out.stdout,
                stderr: out.stderr,
            };
        }
    }

    finish(out)
}

/// A host with the `typecheck` plugin registered — the extension consumes
/// the same `DefaultHost`/`CapabilityRequest` machinery the built-in
/// command did; only the packaging changed (ADR-0149 §Consequences).
fn tooling_host() -> Result<DefaultHost, PlatformError> {
    let mut host = DefaultHost::new();
    host.register_plugin(handl_plugin_ext_typecheck::TypecheckPlugin::new(
        Arc::new(|src, prelude| {
            with_tooling_kit(|kit| kit.typecheck_handl_with_prelude(src, prelude))
        }),
    ));
    host.load()?;
    host.resolve_composition()
        .map_err(|e| PlatformError::Plugin(format!("tooling composition: {e}")))?;
    Ok(host)
}

/// Resolve `path` (possibly relative) against the request's `cwd`.
fn resolve(cwd: &Path, path: &str) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        cwd.join(p)
    }
}

/// Typecheck a single source. Returns 2 on parse error; sets
/// `out.had_failure` for Error diagnostics.
///
/// The canonical-dialect path always routes through `Kit::typecheck_handl`
/// (the Handl-authored `.hnl` model adapter, ADR-0105) via the `typecheck`
/// capability. The dialect routing (Contract/Arrow) is unchanged — only
/// canonical sources reach the Handl engine.
fn typecheck_one(
    host: &mut DefaultHost,
    module_root: Option<&Path>,
    src: &str,
    path: &str,
    out: &mut Out,
) -> i32 {
    // Route non-canonical dialects to their own loaders (ADR-0077).
    match detect_dialect(src) {
        Dialect::Contract => {
            return match handl_plugin_ext_refinement::parse_contracts(src) {
                Ok(_) => 0,
                Err(e) => {
                    out.stderr.push_str(&format!("{path}: contract error: {e}\n"));
                    2
                }
            };
        }
        Dialect::Arrow => {
            // A successful dialect parse means the file is valid in the
            // dialect that owns it; `hnl typecheck` is the canonical
            // checker, so arrow modules are accepted as-is.
            return 0;
        }
        Dialect::Canonical => {}
    }

    // Splice-form file imports (`import "<spec>"`, `from "<spec>" import *`)
    // bind the module's exports into scope at eval time — seed those names
    // so the source-only scope check sees them (same seeding as `hnl run`).
    let bases = spec_resolution_bases(Path::new(path), module_root);
    let deps = handl_platform::discover_dependencies(Path::new(path)).unwrap_or_default();
    let spliced: Vec<String> = spliced_import_names(src, &bases, &deps)
        .into_iter()
        .collect();
    // Same capability path a third-party consumer uses (ADR-0085 step 4).
    let request = CapabilityRequest {
        capability: "typecheck".into(),
        payload: json!({ "source": src, "prelude": spliced }),
    };
    let response = match host.run(&request) {
        Ok(r) => r,
        Err(e) => {
            out.stderr.push_str(&format!("{path}: {e}\n"));
            return 2;
        }
    };
    match response.0.get("status").and_then(|v| v.as_str()) {
        Some("ok") => {
            let typ = response
                .0
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("<unknown>");
            out.stdout.push_str(&format!("{path}: ok: {typ}\n"));
            0
        }
        // Reported type errors: stdout + exit 0, with the failure recorded —
        // matching the pre-reroute behaviour of the `typecheck errors:` arm.
        Some("type-errors") => {
            let msg = response
                .0
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("typecheck failed");
            out.stdout.push_str(&format!("{path}: {msg}\n"));
            out.had_failure = true;
            0
        }
        // Lex / parse / language errors: stderr + exit 2.
        Some("error") => {
            let msg = response
                .0
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("typecheck failed");
            out.stderr.push_str(&format!("{path}: {msg}\n"));
            2
        }
        other => {
            out.stderr
                .push_str(&format!("{path}: unexpected typecheck status {other:?}\n"));
            2
        }
    }
}

// ---------------------------------------------------------------------------
// Import-specifier resolution (shared with `run`/`lint`)
// ---------------------------------------------------------------------------

/// Resolution bases for `import "<spec>"` specifiers in a file at `path`,
/// mirroring `ModuleLoader` order: the file's directory, the module root,
/// then the process working directory. Used by `run`, `typecheck`, and
/// `lint` alike so each command resolves imports the way eval will.
fn spec_resolution_bases(path: &Path, module_root: Option<&Path>) -> Vec<PathBuf> {
    let mut bases: Vec<PathBuf> = Vec::new();
    match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => bases.push(dir.to_path_buf()),
        _ => bases.push(PathBuf::from(".")),
    }
    if let Some(root) = module_root {
        if !bases.iter().any(|b| b.as_path() == root) {
            bases.push(root.to_path_buf());
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        if !bases.iter().any(|b| *b == cwd) {
            bases.push(cwd);
        }
    }
    bases
}

/// Resolve a `[dependencies]` specifier: when the first `/`-separated
/// segment of `spec` names a declared dependency, the remainder resolves
/// inside that package's directory (`dep/lib/file` →
/// `<deps["dep"]>/lib/file`; a bare `dep` name resolves the package dir's
/// `index.hnl`/`<name>.hnl` main file). Returns `None` for non-dep specs.
fn resolve_dep_spec_path(
    spec: &str,
    deps: &std::collections::HashMap<String, PathBuf>,
) -> Option<PathBuf> {
    let (first, rest) = match spec.split_once('/') {
        Some((f, r)) => (f, Some(r)),
        None => (spec, None),
    };
    let dep_dir = deps.get(first)?;
    let joined = match rest {
        Some(r) if !r.is_empty() => dep_dir.join(r),
        _ => dep_dir.clone(),
    };
    file_candidate(&joined, spec)
}

/// Resolve `<spec>` to the file `ModuleLoader` would load: dependency-name
/// resolution first, then `bases` (same order as [`spec_resolves_to_file`]).
fn resolve_spec_path(
    spec: &str,
    bases: &[PathBuf],
    deps: &std::collections::HashMap<String, PathBuf>,
) -> Option<PathBuf> {
    if Path::new(spec).is_absolute() {
        return file_candidate(Path::new(spec), spec);
    }
    resolve_dep_spec_path(spec, deps)
        .or_else(|| bases.iter().find_map(|base| file_candidate(&base.join(spec), spec)))
}

/// The specifier of a *splice-form* import — `import "<spec>"` or
/// `from "<spec>" import *` with no alias — whose exports land directly in
/// the importer's scope at eval time. Named/aliased `from` forms bind
/// statically known names and need no spliced scope.
fn splice_import_spec(trimmed: &str) -> Option<String> {
    if let Some(rest) = trimmed.strip_prefix("import") {
        if !rest.starts_with(' ') && !rest.starts_with('\t') {
            return None;
        }
        let inner = rest.trim_start();
        if !inner.starts_with('"') && !inner.starts_with('\'') {
            return None;
        }
        return first_quoted_spec(inner).map(str::to_string);
    }
    let rest = trimmed.strip_prefix("from")?;
    if !rest.starts_with(' ') && !rest.starts_with('\t') {
        return None;
    }
    // `from "<spec>" <tail>` — splice iff the tail is `import *` exactly.
    let bytes = rest.as_bytes();
    let q = bytes.iter().position(|b| *b == b'"' || *b == b'\'')?;
    let quote = bytes[q] as char;
    let after_q = &rest[q + 1..];
    let end = after_q.find(quote)?;
    let spec = &after_q[..end];
    let tail = after_q[end + quote.len_utf8()..].trim_start();
    let tail = tail.strip_prefix("import")?.trim_start();
    let tail = tail
        .split("--")
        .next()
        .unwrap_or("")
        .trim_end()
        .trim_end_matches(';')
        .trim_end();
    (tail == "*").then(|| spec.to_string())
}

/// Names a file's splice-form imports bind into scope at eval time: for
/// each resolvable `import "<spec>"` / `from "<spec>" import *`, the
/// module's declared exports (collected without evaluating it). The
/// bootstrap typechecker's module scope is source-only, so `hnl run`,
/// `hnl typecheck`, and `hnl lint` seed these into their scope-check
/// prelude — otherwise every use of a spliced export is reported as
/// `unbound wire` even though eval binds it.
fn spliced_import_names(
    src: &str,
    bases: &[PathBuf],
    deps: &std::collections::HashMap<String, PathBuf>,
) -> HashSet<String> {
    let mut names = HashSet::new();
    for line in src.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("--") {
            continue;
        }
        let Some(spec) = splice_import_spec(trimmed) else {
            continue;
        };
        let Some(path) = resolve_spec_path(&spec, bases, deps) else {
            continue;
        };
        let Ok(mod_src) = std::fs::read_to_string(&path) else {
            continue;
        };
        names.extend(handl_runtime::module_prep::module_export_names(&mod_src));
    }
    names
}

fn file_candidate(joined: &Path, spec: &str) -> Option<PathBuf> {
    if joined.is_dir() {
        let index = joined.join("index.hnl");
        if index.is_file() {
            return Some(index);
        }
        if let Some(name) = joined.file_name().and_then(|s| s.to_str()) {
            let main = joined.join(format!("{}.hnl", name));
            if main.is_file() {
                return Some(main);
            }
        }
    }
    if !spec.ends_with(".hnl") {
        let with_ext = joined.with_extension("hnl");
        if with_ext.is_file() {
            return Some(with_ext);
        }
    }
    if joined.is_file() {
        Some(joined.to_path_buf())
    } else {
        None
    }
}

/// Extract the first `'`- or `"`-quoted string literal's contents from a
/// source fragment (e.g. the specifier in `from "..." import ...`).
fn first_quoted_spec(s: &str) -> Option<&str> {
    let bytes = s.as_bytes();
    let start = bytes.iter().position(|&b| b == b'"' || b == b'\'')?;
    let quote = bytes[start];
    let rest = &s[start + 1..];
    let end = rest.as_bytes().iter().position(|&b| b == quote)?;
    Some(&rest[..end])
}

// ---------------------------------------------------------------------------
// Response plumbing
// ---------------------------------------------------------------------------

fn err(msg: impl Into<String>) -> ExtResponse {
    ExtResponse::error(2, msg.into())
}

fn finish(out: Out) -> ExtResponse {
    ExtResponse {
        code: if out.had_failure { 1 } else { 0 },
        stdout: out.stdout,
        stderr: out.stderr,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    /// A unique temp dir per test.
    fn tmpdir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("hnl-ext-typecheck-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn unknown_flag_is_usage_error() {
        let res = run(&["--bogus".into()], Path::new("."));
        assert_eq!(res.code, 2);
        assert!(res.stderr.contains("typecheck: unknown flag `--bogus`"));
    }

    #[test]
    fn missing_file_is_read_error() {
        let dir = tmpdir("missing");
        let res = run(&["nope.hnl".into()], &dir);
        assert_eq!(res.code, 2);
        assert!(res.stderr.contains("typecheck: failed to read"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn typechecks_a_simple_file_ok() {
        let dir = tmpdir("ok");
        let file = dir.join("ok.hnl");
        std::fs::write(&file, "x = 1;").unwrap();
        let res = run(&[file.to_string_lossy().into_owned()], &dir);
        assert_eq!(res.code, 0, "stdout: {} stderr: {}", res.stdout, res.stderr);
        assert!(res.stdout.contains(": ok:"), "stdout: {}", res.stdout);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The `export!`-generated manifest symbol, called directly.
    #[test]
    fn manifest_declares_typecheck_and_check() {
        let raw = handl_ext_manifest();
        assert!(!raw.is_null());
        let json = unsafe { CStr::from_ptr(raw) }
            .to_string_lossy()
            .into_owned();
        unsafe { handl_ext_free(raw) };
        let manifest: ExtensionManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(manifest.name, "typecheck");
        let names: Vec<&str> = manifest.commands.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["typecheck", "check"]);
    }
}

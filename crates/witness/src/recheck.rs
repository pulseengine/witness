//! `witness recheck` (v0.47, FEAT-045): re-check an evidence bundle without
//! access to the build.
//!
//! Three kinds of check, each re-computed from the bytes in the bundle (no
//! digest is trusted that recheck did not compute itself):
//!
//! - **(a) inventory.** When `<dir>/INVENTORY.sha256` is present, every file
//!   it lists must exist and hash to the listed digest, and every regular file
//!   under `<dir>` must be listed (the inventory itself is the only
//!   exclusion). Symlinks and other non-regular entries fail.
//! - **(b) hash chain.** Each hash link the producers record is re-checked
//!   from bytes: source module → manifest, instrumented module → run record,
//!   manifest → run record, run record → predicate (`measurement.run_sha256`,
//!   plus the branch-summary coverage section), instrumented / source module
//!   → predicate subjects, predicate → DSSE payload, and the DSSE signature
//!   against a key given on the command line. With `--expect-key`, the
//!   predicate links are checked against the statement that verifies, and a
//!   missing envelope or predicate fails. A link no producer records is
//!   reported NOT-CHECKED, never PASS.
//! - **(c) re-derivation.** `report.json` is re-derived from `run.json`
//!   through the reporter `witness report --format mcdc-json` uses
//!   ([`McdcReport::to_json_document`]) and must be byte-equal.
//!
//! Directory rule: if `<dir>` itself holds any verdict file (`run.json`,
//! `report.json`, `manifest.json`, `instrumented.wasm`, `predicate.json`,
//! `signed.dsse.json`) it is one verdict, and a subdirectory that also holds
//! verdict files fails (`ambiguous-layout`). Otherwise every immediate
//! subdirectory is a verdict, a directory with none fails, and a top-level
//! file must be one run-suite.sh writes (`INVENTORY.sha256`,
//! `verifying-key.pub`, `VERIFY.md`, `SUMMARY.txt`) or be listed in the
//! inventory (`unexpected-top-level-file`).
//!
//! No network, no build access, deterministic output. Exit codes (set by the
//! CLI): 0 when no check failed, 1 when any check failed, 2 when recheck could
//! not run (unreadable directory, malformed `--expect-key`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use serde::Serialize;
use serde_json::Value;
use witness_core::mcdc_report::McdcReport;
use witness_core::predicate::{
    MCDC_PREDICATE_TYPE, MCDC_PREDICATE_TYPE_V2, MCDC_PREDICATE_TYPE_V3, PREDICATE_TYPE,
    sha256_hex_pub,
};
use witness_core::run_record::RunRecord;

/// Inventory file name, at the root of the directory given to recheck.
/// Format: one line per file, `<64 lowercase hex sha256>  <relative path>`
/// (two spaces, `/`-separated, LF line ends), sorted by path when written by
/// [`write_inventory`]. Compatible with `sha256sum -c` run from that root.
pub const INVENTORY_FILE: &str = "INVENTORY.sha256";

/// `schema` of the `--format json` output.
pub const RECHECK_SCHEMA: &str = "https://pulseengine.eu/witness-recheck/v1";

/// What a passing recheck shows, and what it does not. Printed on every run.
pub const SCOPE: &str = "recheck shows that each report.json follows from the shipped run.json \
through witness's own reporter, and that the bundle is internally consistent (inventory and \
recorded hashes). It does NOT show that run.json came from a real execution, or that witness \
itself is correct.";

const VERDICT_MARKERS: &[&str] = &[
    "run.json",
    "report.json",
    "manifest.json",
    "instrumented.wasm",
    "predicate.json",
    "signed.dsse.json",
];
const IN_BUNDLE_KEY: &str = "verifying-key.pub";

/// Result of one check.
#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Pass,
    Fail,
    NotChecked,
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Status::Pass => "PASS",
            Status::Fail => "FAIL",
            Status::NotChecked => "NOT-CHECKED",
        }
    }
}

/// One line of recheck output.
#[derive(Serialize, Clone, Debug)]
pub struct Check {
    /// Stable check name, `<verdict>/<check>` or `inventory`.
    pub id: String,
    pub status: Status,
    /// Set on failures only: the failure class (distinct per kind of failure).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_class: Option<&'static str>,
    pub reason: String,
}

#[derive(Serialize, Default, Debug)]
pub struct Summary {
    pub pass: usize,
    pub fail: usize,
    pub not_checked: usize,
}

/// The whole recheck result (`--format json` serialises this).
#[derive(Serialize, Debug)]
pub struct Outcome {
    pub schema: &'static str,
    pub dir: String,
    /// True when no check failed. NOT-CHECKED lines do not make this false.
    pub ok: bool,
    pub scope: &'static str,
    pub summary: Summary,
    pub checks: Vec<Check>,
}

impl Outcome {
    /// The distinct failure classes, sorted.
    pub fn failure_classes(&self) -> BTreeSet<&'static str> {
        self.checks.iter().filter_map(|c| c.failure_class).collect()
    }

    /// Human-readable report: scope, one line per check, a verdict line.
    pub fn to_text(&self) -> String {
        let mut out = format!("witness recheck {}\nscope: {}\n", self.dir, SCOPE);
        for c in &self.checks {
            let class = c
                .failure_class
                .map(|k| format!("[{k}] "))
                .unwrap_or_default();
            out.push_str(&format!(
                "{:<11}  {}  {}{}\n",
                c.status.label(),
                c.id,
                class,
                c.reason
            ));
        }
        let classes: Vec<&str> = self.failure_classes().into_iter().collect();
        out.push_str(&format!(
            "recheck: {}: {} passed, {} failed, {} not checked{}\n",
            if self.ok { "OK" } else { "FAILED" },
            self.summary.pass,
            self.summary.fail,
            self.summary.not_checked,
            if classes.is_empty() {
                String::new()
            } else {
                format!("; failure classes: {}", classes.join(", "))
            }
        ));
        out
    }
}

/// A public key given on the command line: the trust anchor for the DSSE
/// signature check. Never read from the bundle.
pub struct ExpectedKey {
    pub path: String,
    pub bytes: Vec<u8>,
}

impl ExpectedKey {
    /// Read a raw 32-byte Ed25519 public key (the `witness keygen --public` format).
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let bytes = std::fs::read(path)
            .with_context(|| format!("cannot read --expect-key {}", path.display()))?;
        if bytes.len() != 32 {
            bail!(
                "--expect-key {} is {} bytes; expected a raw 32-byte Ed25519 public key",
                path.display(),
                bytes.len()
            );
        }
        Ok(Self {
            path: path.display().to_string(),
            bytes,
        })
    }
}

#[derive(Default)]
struct Checks(Vec<Check>);

impl Checks {
    fn push(&mut self, id: String, status: Status, class: Option<&'static str>, reason: String) {
        self.0.push(Check {
            id,
            status,
            failure_class: class,
            reason,
        });
    }
    fn pass(&mut self, id: String, reason: impl Into<String>) {
        self.push(id, Status::Pass, None, reason.into());
    }
    fn fail(&mut self, id: String, class: &'static str, reason: impl Into<String>) {
        self.push(id, Status::Fail, Some(class), reason.into());
    }
    fn skip(&mut self, id: String, reason: impl Into<String>) {
        self.push(id, Status::NotChecked, None, reason.into());
    }
}

/// Run every check on `dir`. `Err` only when recheck cannot run at all.
pub fn recheck(dir: &Path, expected_key: Option<&ExpectedKey>) -> anyhow::Result<Outcome> {
    let meta = std::fs::metadata(dir).with_context(|| format!("cannot read {}", dir.display()))?;
    if !meta.is_dir() {
        bail!("{} is not a directory", dir.display());
    }
    let mut checks = Checks::default();
    let listed = check_inventory(dir, &mut checks)?;
    let verdicts = verdict_dirs(dir, listed.as_ref(), &mut checks)?;
    if verdicts.is_empty() {
        checks.fail(
            "verdicts".to_string(),
            "no-verdicts",
            format!(
                "{} holds no verdict files and no verdict subdirectories; nothing was checked",
                dir.display()
            ),
        );
    }
    for (label, vdir) in &verdicts {
        check_verdict(label, vdir, dir, expected_key, &mut checks);
    }
    let mut summary = Summary::default();
    for c in &checks.0 {
        let slot = match c.status {
            Status::Pass => &mut summary.pass,
            Status::Fail => &mut summary.fail,
            Status::NotChecked => &mut summary.not_checked,
        };
        *slot = slot.saturating_add(1);
    }
    Ok(Outcome {
        schema: RECHECK_SCHEMA,
        dir: dir.display().to_string(),
        ok: summary.fail == 0,
        scope: SCOPE,
        summary,
        checks: checks.0,
    })
}

/// Write `<dir>/INVENTORY.sha256` covering every regular file under `dir`
/// (except an existing inventory, which is replaced). Returns the file count.
pub fn write_inventory(dir: &Path) -> anyhow::Result<usize> {
    let (files, odd) = list_files(dir)?;
    if let Some(first) = odd.first() {
        bail!("cannot inventory {}: {first}", dir.display());
    }
    let mut text = String::new();
    let mut n: usize = 0;
    for rel in files.iter().filter(|f| f.as_str() != INVENTORY_FILE) {
        let bytes = std::fs::read(dir.join(rel))
            .with_context(|| format!("cannot read {}", dir.join(rel).display()))?;
        text.push_str(&sha256_hex_pub(&bytes));
        text.push_str("  ");
        text.push_str(rel);
        text.push('\n');
        n = n.saturating_add(1);
    }
    std::fs::write(dir.join(INVENTORY_FILE), text)
        .with_context(|| format!("cannot write {}", dir.join(INVENTORY_FILE).display()))?;
    Ok(n)
}

// ---------------------------------------------------------------------------
// (a) inventory
// ---------------------------------------------------------------------------

/// Every regular file under `root` as a sorted `/`-joined relative path, plus
/// a description of each entry an inventory cannot cover (symlink, special
/// file, non-UTF-8 or newline-bearing name).
fn list_files(root: &Path) -> anyhow::Result<(Vec<String>, Vec<String>)> {
    let mut files = Vec::new();
    let mut odd = Vec::new();
    walk(root, "", &mut files, &mut odd)?;
    files.sort();
    Ok((files, odd))
}

fn walk(
    dir: &Path,
    prefix: &str,
    files: &mut Vec<String>,
    odd: &mut Vec<String>,
) -> anyhow::Result<()> {
    let mut entries: Vec<std::fs::DirEntry> = std::fs::read_dir(dir)
        .with_context(|| format!("cannot list {}", dir.display()))?
        .collect::<std::io::Result<_>>()
        .with_context(|| format!("cannot list {}", dir.display()))?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for e in entries {
        let name = e.file_name();
        let Some(name) = name
            .to_str()
            .filter(|n| !n.contains('\n') && !n.contains('\\'))
        else {
            odd.push(format!(
                "{prefix}{}: name is not representable in an inventory",
                name.to_string_lossy()
            ));
            continue;
        };
        let rel = format!("{prefix}{name}");
        let ft = e
            .file_type()
            .with_context(|| format!("cannot stat {}", e.path().display()))?;
        if ft.is_symlink() {
            odd.push(format!("{rel}: is a symlink"));
        } else if ft.is_dir() {
            walk(&e.path(), &format!("{rel}/"), files, odd)?;
        } else if ft.is_file() {
            files.push(rel);
        } else {
            odd.push(format!("{rel}: is not a regular file"));
        }
    }
    Ok(())
}

/// Parse an inventory: `path → digest`. Errors name the offending line.
fn parse_inventory(text: &str) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    for (i, line) in text.lines().enumerate() {
        let n = i.saturating_add(1);
        let Some((digest, path)) = line.split_once("  ") else {
            return Err(format!("line {n}: expected `<sha256>  <path>`"));
        };
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(format!(
                "line {n}: `{digest}` is not a lowercase hex sha256"
            ));
        }
        if path.is_empty()
            || path.starts_with('/')
            || path.contains('\\')
            || path.contains('\r')
            || path
                .split('/')
                .any(|c| c.is_empty() || c == "." || c == "..")
        {
            return Err(format!("line {n}: `{path}` is not a plain relative path"));
        }
        if out.insert(path.to_string(), digest.to_string()).is_some() {
            return Err(format!("line {n}: `{path}` is listed twice"));
        }
    }
    Ok(out)
}

/// Returns the listed paths when an inventory was present and parsed.
fn check_inventory(root: &Path, checks: &mut Checks) -> anyhow::Result<Option<BTreeSet<String>>> {
    let id = || "inventory".to_string();
    let inv = root.join(INVENTORY_FILE);
    match std::fs::symlink_metadata(&inv) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            checks.skip(
                id(),
                format!(
                    "no {INVENTORY_FILE} in {}; write one before archiving with \
                     `witness recheck --write-inventory <dir>`",
                    root.display()
                ),
            );
            return Ok(None);
        }
        Err(e) => return Err(e).with_context(|| format!("cannot stat {}", inv.display())),
        Ok(m) if !m.is_file() => {
            checks.fail(
                id(),
                "inventory-malformed",
                format!("{INVENTORY_FILE} is not a regular file"),
            );
            return Ok(None);
        }
        Ok(_) => {}
    }
    let bytes = std::fs::read(&inv).with_context(|| format!("cannot read {}", inv.display()))?;
    let listed = match std::str::from_utf8(&bytes)
        .map_err(|_| "not UTF-8".to_string())
        .and_then(parse_inventory)
    {
        Ok(l) => l,
        Err(e) => {
            checks.fail(
                id(),
                "inventory-malformed",
                format!("{INVENTORY_FILE}: {e}"),
            );
            return Ok(None);
        }
    };
    let (files, odd) = list_files(root)?;
    let on_disk: BTreeSet<&str> = files
        .iter()
        .map(String::as_str)
        .filter(|f| *f != INVENTORY_FILE)
        .collect();
    let before = checks.0.len();
    for o in &odd {
        checks.fail(id(), "inventory-unsupported-entry", o.clone());
    }
    for (path, digest) in &listed {
        if !on_disk.contains(path.as_str()) {
            checks.fail(
                id(),
                "inventory-missing-file",
                format!("{INVENTORY_FILE} lists {path}, which is not in the bundle"),
            );
            continue;
        }
        let content = std::fs::read(root.join(path))
            .with_context(|| format!("cannot read {}", root.join(path).display()))?;
        let actual = sha256_hex_pub(&content);
        if &actual != digest {
            checks.fail(
                id(),
                "inventory-digest-mismatch",
                format!("{path}: content hashes to {actual}, {INVENTORY_FILE} lists {digest}"),
            );
        }
    }
    for f in &on_disk {
        if !listed.contains_key(*f) {
            checks.fail(
                id(),
                "inventory-unlisted-file",
                format!("{f} is in the bundle but not listed in {INVENTORY_FILE}"),
            );
        }
    }
    if checks.0.len() == before {
        checks.pass(
            id(),
            format!(
                "{} files re-hashed: every listed file is present with matching content, \
                 and every file is listed",
                listed.len()
            ),
        );
    }
    Ok(Some(listed.into_keys().collect()))
}

// ---------------------------------------------------------------------------
// verdicts: (b) hash chain and (c) re-derivation
// ---------------------------------------------------------------------------

/// Top-level files a suite directory may hold without being inventoried
/// (what verdicts/run-suite.sh writes next to the verdict directories).
const SUITE_TOP_LEVEL_FILES: &[&str] = &[INVENTORY_FILE, IN_BUNDLE_KEY, "VERIFY.md", "SUMMARY.txt"];

fn has_markers(dir: &Path) -> bool {
    VERDICT_MARKERS.iter().any(|m| dir.join(m).exists())
}

/// Decide which directories are verdicts. A root that holds verdict files is
/// one verdict; if any subdirectory also holds verdict files the layout is
/// ambiguous and fails (root files must not hide subdirectory verdicts).
/// Otherwise every subdirectory is a verdict, and each top-level file must be
/// one run-suite.sh writes or be listed in the inventory.
fn verdict_dirs(
    root: &Path,
    listed: Option<&BTreeSet<String>>,
    checks: &mut Checks,
) -> anyhow::Result<Vec<(String, PathBuf)>> {
    let mut subdirs = Vec::new();
    let mut top_files = Vec::new();
    for e in std::fs::read_dir(root).with_context(|| format!("cannot list {}", root.display()))? {
        let e = e.with_context(|| format!("cannot list {}", root.display()))?;
        let name = e.file_name().to_string_lossy().into_owned();
        if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            subdirs.push((name, e.path()));
        } else {
            top_files.push(name);
        }
    }
    subdirs.sort();
    top_files.sort();
    if has_markers(root) {
        let nested: Vec<&str> = subdirs
            .iter()
            .filter(|(_, p)| has_markers(p))
            .map(|(n, _)| n.as_str())
            .collect();
        if !nested.is_empty() {
            checks.fail(
                "layout".to_string(),
                "ambiguous-layout",
                format!(
                    "{} holds verdict files and so do its subdirectories {}; recheck one \
                     verdict or one suite, not both",
                    root.display(),
                    nested.join(", ")
                ),
            );
        }
        let label = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| ".".to_string());
        return Ok(vec![(label, root.to_path_buf())]);
    }
    for f in &top_files {
        let known = SUITE_TOP_LEVEL_FILES.contains(&f.as_str());
        let inventoried = listed.is_some_and(|l| l.contains(f));
        if !known && !inventoried {
            checks.fail(
                "layout".to_string(),
                "unexpected-top-level-file",
                format!(
                    "{f} sits at the top of a suite directory, is not a file run-suite.sh \
                     writes there, and is not listed in {INVENTORY_FILE}"
                ),
            );
        }
    }
    Ok(subdirs)
}

/// Read `dir/name` if it exists. A read error other than not-found is
/// recorded as a failure and treated as absent.
fn read_opt(dir: &Path, name: &str, id: &str, checks: &mut Checks) -> Option<Vec<u8>> {
    match std::fs::read(dir.join(name)) {
        Ok(b) => Some(b),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            checks.fail(
                format!("{id}/{name}"),
                "unreadable-file",
                format!("cannot read {name}: {e}"),
            );
            None
        }
    }
}

fn parse_json(bytes: &[u8], name: &str, id: String, checks: &mut Checks) -> Option<Value> {
    match serde_json::from_slice(bytes) {
        Ok(v) => Some(v),
        Err(e) => {
            checks.fail(
                id,
                "unparsable-json",
                format!("{name} is not valid JSON: {e}"),
            );
            None
        }
    }
}

fn short(d: &str) -> &str {
    d.get(..16).unwrap_or(d)
}

/// One recorded-hash link: `recorded` is what a file in the bundle claims
/// `sha256(target)` is; `actual` is recheck's own hash of `target`.
fn link(
    checks: &mut Checks,
    id: String,
    recorded: Option<&str>,
    recorded_where: &str,
    target: &str,
    actual: Option<&str>,
    not_recorded: &str,
) {
    let Some(rec) = recorded else {
        checks.skip(id, not_recorded);
        return;
    };
    match actual {
        None => checks.fail(
            id,
            "chain-target-missing",
            format!("{recorded_where} records sha256 {} for {target}, but {target} is not in the bundle", short(rec)),
        ),
        Some(act) if act.eq_ignore_ascii_case(rec) => checks.pass(
            id,
            format!("sha256({target}) = {}... matches {recorded_where}", short(act)),
        ),
        Some(act) => checks.fail(
            id,
            "chain-mismatch",
            format!("sha256({target}) = {act}, but {recorded_where} records {rec}"),
        ),
    }
}

/// Drop the `module` key: it carries a path whose form depends on the
/// producer's working directory, not on the evidence.
fn without_module(v: &Value) -> Value {
    let mut v = v.clone();
    if let Some(o) = v.as_object_mut() {
        o.remove("module");
    }
    v
}

/// First differing line between `shipped` and `derived`, for the reason text.
fn first_difference(shipped: &[u8], derived: &str) -> String {
    let shipped = String::from_utf8_lossy(shipped);
    let clip = |s: &str| -> String { s.trim().chars().take(100).collect() };
    for (i, (a, b)) in shipped.lines().zip(derived.lines()).enumerate() {
        if a != b {
            return format!(
                "first difference at line {}: shipped `{}`, re-derived `{}`",
                i.saturating_add(1),
                clip(a),
                clip(b)
            );
        }
    }
    format!(
        "shipped is {} bytes, re-derived is {} bytes",
        shipped.len(),
        derived.len()
    )
}

fn check_verdict(
    label: &str,
    vdir: &Path,
    root: &Path,
    expected_key: Option<&ExpectedKey>,
    checks: &mut Checks,
) {
    let id = |s: &str| format!("{label}/{s}");

    // Required inputs for (c).
    let run_bytes = read_opt(vdir, "run.json", label, checks);
    let record: Option<RunRecord> = match &run_bytes {
        None => {
            checks.fail(
                id("run.json"),
                "required-file-missing",
                "run.json is missing: there is no run record to re-derive the report from",
            );
            None
        }
        Some(b) => match serde_json::from_slice(b) {
            Ok(r) => Some(r),
            Err(e) => {
                checks.fail(
                    id("run.json"),
                    "run-record-unreadable",
                    format!("run.json is not a witness run record: {e}"),
                );
                None
            }
        },
    };
    let report_bytes = read_opt(vdir, "report.json", label, checks);
    if report_bytes.is_none() {
        checks.fail(
            id("report.json"),
            "required-file-missing",
            "report.json is missing",
        );
    }

    // (c) re-derive report.json from run.json.
    match (&record, &report_bytes) {
        (Some(r), Some(shipped)) => match McdcReport::from_record(r).to_json_document() {
            Ok(derived) if derived.as_bytes() == shipped.as_slice() => checks.pass(
                id("rederive-report"),
                format!(
                    "report.json ({} bytes) is byte-equal to the report re-derived from run.json \
                     through the witness-mcdc-checker kernel (the run -> report link)",
                    shipped.len()
                ),
            ),
            Ok(derived) => checks.fail(
                id("rederive-report"),
                "rederive-mismatch",
                format!(
                    "report.json does not follow from run.json: {}",
                    first_difference(shipped, &derived)
                ),
            ),
            Err(e) => checks.fail(
                id("rederive-report"),
                "rederive-error",
                format!("cannot serialise the re-derived report: {e}"),
            ),
        },
        _ => checks.skip(
            id("rederive-report"),
            "needs a readable run.json and report.json",
        ),
    }

    // (b) hash chain. Hash every file a link may point at, from its bytes.
    let module = read_opt(vdir, "instrumented.wasm", label, checks).map(|b| sha256_hex_pub(&b));
    let source = read_opt(vdir, "source.wasm", label, checks).map(|b| sha256_hex_pub(&b));
    let manifest_bytes = read_opt(vdir, "manifest.json", label, checks);
    let manifest_sha = manifest_bytes.as_deref().map(sha256_hex_pub);
    let manifest = manifest_bytes
        .as_deref()
        .and_then(|b| parse_json(b, "manifest.json", id("manifest.json"), checks));
    let predicate_bytes = read_opt(vdir, "predicate.json", label, checks);
    let predicate = predicate_bytes
        .as_deref()
        .and_then(|b| parse_json(b, "predicate.json", id("predicate.json"), checks));
    let envelope_bytes = read_opt(vdir, "signed.dsse.json", label, checks);

    // source.wasm -> manifest
    match &manifest {
        None => checks.skip(id("chain/source->manifest"), "no readable manifest.json"),
        Some(m) => link(
            checks,
            id("chain/source->manifest"),
            m.get("original_module_sha256").and_then(Value::as_str),
            "manifest.json original_module_sha256",
            "source.wasm",
            source.as_deref(),
            "manifest.json records no original_module_sha256",
        ),
    }

    // instrumented.wasm -> run.json, manifest.json -> run.json
    match &record {
        None => {
            checks.skip(id("chain/module->run"), "no readable run.json");
            checks.skip(id("chain/manifest->run"), "no readable run.json");
        }
        Some(r) => {
            link(
                checks,
                id("chain/module->run"),
                r.module_sha256.as_deref(),
                "run.json module_sha256",
                "instrumented.wasm",
                module.as_deref(),
                "run.json records no module_sha256 (written by witness >= 0.47 only)",
            );
            link(
                checks,
                id("chain/manifest->run"),
                r.manifest_sha256.as_deref(),
                "run.json manifest_sha256",
                "manifest.json",
                manifest_sha.as_deref(),
                "run.json records no manifest_sha256 (written by witness >= 0.47 only)",
            );
        }
    }
    // Signature first, against a key from outside the bundle only: with an
    // expected signer, the statement that verifies is the one every predicate
    // link is checked against, never the unsigned predicate.json.
    let sig_id = id("signature");
    let mut signed: Option<Value> = None;
    match (expected_key, &envelope_bytes) {
        (None, _) => checks.skip(
            sig_id,
            "no --expect-key given; the in-bundle verifying-key.pub is not a trust anchor, \
             and the keyless cosign certificate identity check is not implemented",
        ),
        (Some(k), None) => checks.fail(
            sig_id,
            "signature-missing",
            format!(
                "--expect-key {} given, but signed.dsse.json is missing",
                k.path
            ),
        ),
        (Some(k), Some(env)) => match witness_core::attest::verify_envelope(env, &k.bytes) {
            Ok(stmt) => match serde_json::to_value(&stmt) {
                Ok(v) => {
                    checks.pass(
                        sig_id,
                        format!(
                            "signed.dsse.json verifies against the expected key {}",
                            k.path
                        ),
                    );
                    signed = Some(v);
                }
                Err(e) => checks.fail(
                    sig_id,
                    "envelope-malformed",
                    format!("the signed statement cannot be re-read: {e}"),
                ),
            },
            Err(e) => {
                let hint = if in_bundle_keys(vdir, root)
                    .iter()
                    .any(|b| witness_core::attest::verify_envelope(env, b).is_ok())
                {
                    "; it does verify against the in-bundle verifying-key.pub, so it was signed \
                     by a key other than the expected signer"
                } else {
                    ""
                };
                checks.fail(
                    sig_id,
                    "signer-mismatch",
                    format!(
                        "signed.dsse.json does not verify against the expected key {} ({e}){hint}",
                        k.path
                    ),
                );
            }
        },
    }

    // predicate.json against the DSSE payload (compared as JSON values).
    let pe_id = id("chain/predicate->envelope");
    match (&signed, &predicate, &envelope_bytes) {
        (Some(s), Some(p), _) if s == p => checks.pass(
            pe_id,
            "predicate.json is the signed statement (compared as JSON values)",
        ),
        (Some(_), Some(_), _) => checks.fail(
            pe_id,
            "envelope-payload-mismatch",
            "predicate.json is not the statement signed in signed.dsse.json (compared as JSON \
             values); the predicate links below are checked against the signed statement",
        ),
        (Some(_), None, _) => checks.fail(
            pe_id,
            "predicate-missing",
            "--expect-key given, but predicate.json is missing or unreadable",
        ),
        (None, _, None) => checks.skip(pe_id, "no signed.dsse.json"),
        (None, None, Some(_)) => checks.skip(
            pe_id,
            "no readable predicate.json to compare the payload with",
        ),
        (None, Some(p), Some(env)) => match envelope_payload(env) {
            Ok(payload) if &payload == p => checks.pass(
                pe_id,
                "the DSSE payload is the same statement as predicate.json (compared as JSON \
                 values; signature not checked)",
            ),
            Ok(_) => checks.fail(
                pe_id,
                "envelope-payload-mismatch",
                "the DSSE payload in signed.dsse.json is not the statement in predicate.json",
            ),
            Err(e) => checks.fail(pe_id, "envelope-malformed", e),
        },
    }

    // Predicate links: subjects, and the binding to run.json / report.json.
    let statement = match (&signed, expected_key) {
        (Some(s), _) => Some((s, "signed statement")),
        (None, Some(_)) => None,
        (None, None) => predicate.as_ref().map(|p| (p, "predicate.json (unsigned)")),
    };
    match statement {
        None => {
            let why = if expected_key.is_some() {
                "no statement verifies against the expected key"
            } else {
                "no readable predicate.json"
            };
            for l in [
                "chain/module->predicate",
                "chain/source->predicate",
                "chain/run->predicate",
                "chain/run->predicate-coverage",
                "chain/report->predicate",
            ] {
                checks.skip(id(l), why);
            }
        }
        Some((p, src)) => check_predicate(
            p,
            src,
            &id,
            &PredicateInputs {
                record: record.as_ref(),
                run_sha: run_bytes.as_deref().map(sha256_hex_pub),
                report_bytes: report_bytes.as_deref(),
                module: module.as_deref(),
                source: source.as_deref(),
            },
            checks,
        ),
    }
}

/// Recheck's own hashes and parses of the files a predicate may bind.
struct PredicateInputs<'a> {
    record: Option<&'a RunRecord>,
    run_sha: Option<String>,
    report_bytes: Option<&'a [u8]>,
    module: Option<&'a str>,
    source: Option<&'a str>,
}

/// Check one statement (the signed one when an expected key verified it,
/// else the unsigned predicate.json; `src` names which) against recheck's
/// own hashes of the bundle files.
fn check_predicate(
    p: &Value,
    src: &str,
    id: &dyn Fn(&str) -> String,
    inputs: &PredicateInputs<'_>,
    checks: &mut Checks,
) {
    fn subject_digest(p: &Value, i: usize) -> Option<&str> {
        p.get("subject")?
            .as_array()?
            .get(i)?
            .get("digest")?
            .get("sha256")?
            .as_str()
    }
    // Subject 0 is the instrumented module, subject 1 the original module
    // (the order `witness predicate` writes, predicate.rs).
    link(
        checks,
        id("chain/module->predicate"),
        subject_digest(p, 0),
        &format!("{src} subject[0]"),
        "instrumented.wasm",
        inputs.module,
        &format!("{src} has no subject"),
    );
    link(
        checks,
        id("chain/source->predicate"),
        subject_digest(p, 1),
        &format!("{src} subject[1]"),
        "source.wasm",
        inputs.source,
        &format!("{src} names no original-module subject"),
    );

    // The whole run record, bound by digest (witness >= 0.47).
    let body = p.get("predicate");
    let recorded_run = body
        .and_then(|b| b.get("measurement"))
        .and_then(|m| m.get("run_sha256"))
        .and_then(Value::as_str);
    link(
        checks,
        id("chain/run->predicate"),
        recorded_run,
        &format!("{src} measurement.run_sha256"),
        "run.json",
        inputs.run_sha.as_deref(),
        &format!(
            "{src} records no measurement.run_sha256 (written by witness >= 0.47 only): the \
             MC/DC decisions in run.json, and so report.json, are not covered by any signature"
        ),
    );

    let ptype = p.get("predicateType").and_then(Value::as_str).unwrap_or("");
    if ptype == PREDICATE_TYPE {
        // A branch-count summary only: it never binds the MC/DC rows.
        let cov_id = id("chain/run->predicate-coverage");
        match (inputs.record, body.and_then(|b| b.get("coverage"))) {
            (None, _) => checks.skip(cov_id, "no readable run.json"),
            (Some(_), None) => checks.fail(
                cov_id,
                "predicate-coverage-mismatch",
                format!("{src} (witness-coverage/v1) has no coverage section"),
            ),
            (Some(r), Some(cov)) => {
                let derived = serde_json::to_value(witness_core::report::Report::from_record(r))
                    .unwrap_or(Value::Null);
                if without_module(&derived) == without_module(cov) {
                    checks.pass(
                        cov_id,
                        format!(
                            "{src} coverage section equals the branch summary re-derived from \
                             run.json (module path not compared; a summary, not the MC/DC rows)"
                        ),
                    );
                } else {
                    checks.fail(
                        cov_id,
                        "predicate-coverage-mismatch",
                        format!(
                            "{src} coverage section differs from the branch summary re-derived \
                             from run.json"
                        ),
                    );
                }
            }
        }
        checks.skip(
            id("chain/report->predicate"),
            "witness-coverage/v1 predicates do not embed report.json; it is bound through \
             run->predicate plus rederive-report",
        );
    } else if [
        MCDC_PREDICATE_TYPE,
        MCDC_PREDICATE_TYPE_V2,
        MCDC_PREDICATE_TYPE_V3,
    ]
    .contains(&ptype)
    {
        checks.skip(
            id("chain/run->predicate-coverage"),
            "witness-mcdc predicates carry no branch summary",
        );
        let rid = id("chain/report->predicate");
        let report = body.and_then(|b| b.get("report"));
        let stored = body
            .and_then(|b| b.get("report_sha256"))
            .and_then(Value::as_str);
        match (report, stored) {
            (Some(rep), Some(stored)) => {
                let canonical = serde_json::to_vec(rep).unwrap_or_default();
                let derived = sha256_hex_pub(&canonical);
                let shipped: Option<Value> = inputs
                    .report_bytes
                    .and_then(|b| serde_json::from_slice(b).ok());
                if !derived.eq_ignore_ascii_case(stored) {
                    checks.fail(
                        rid,
                        "chain-mismatch",
                        format!(
                            "sha256 of the report embedded in {src} is {derived}, its \
                             report_sha256 records {stored}"
                        ),
                    );
                } else if shipped.as_ref().map(without_module) != Some(without_module(rep)) {
                    checks.fail(
                        rid,
                        "predicate-report-mismatch",
                        format!(
                            "the report embedded in {src} is not report.json (module path not \
                             compared)"
                        ),
                    );
                } else {
                    checks.pass(
                        rid,
                        format!(
                            "the report embedded in {src} hashes to its report_sha256 {}... and \
                             equals report.json",
                            short(&derived)
                        ),
                    );
                }
            }
            _ => checks.fail(
                rid,
                "chain-mismatch",
                format!("{src} (witness-mcdc) lacks report or report_sha256"),
            ),
        }
    } else {
        for l in ["chain/run->predicate-coverage", "chain/report->predicate"] {
            checks.skip(id(l), format!("unknown predicateType `{ptype}`"));
        }
    }
}

/// The DSSE payload decoded as JSON, without verifying any signature.
fn envelope_payload(env: &[u8]) -> Result<Value, String> {
    let v: Value =
        serde_json::from_slice(env).map_err(|e| format!("signed.dsse.json is not JSON: {e}"))?;
    let b64 = v
        .get("payload")
        .and_then(Value::as_str)
        .ok_or("signed.dsse.json has no payload")?;
    let bytes = B64
        .decode(b64)
        .map_err(|e| format!("signed.dsse.json payload is not base64: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("signed.dsse.json payload is not JSON: {e}"))
}

/// In-bundle public keys, used only to word a diagnostic, never to pass.
fn in_bundle_keys(vdir: &Path, root: &Path) -> Vec<Vec<u8>> {
    [vdir.join(IN_BUNDLE_KEY), root.join(IN_BUNDLE_KEY)]
        .iter()
        .filter_map(|p| std::fs::read(p).ok())
        .collect()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]
mod tests {
    use super::*;

    const D: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    #[test]
    fn inventory_parses_sha256sum_lines() {
        let m = parse_inventory(&format!("{D}  a/b.json\n{D}  c\n")).unwrap();
        assert_eq!(m.len(), 2);
        assert_eq!(m.get("a/b.json").map(String::as_str), Some(D));
    }

    #[test]
    fn inventory_rejects_malformed_lines() {
        assert!(parse_inventory("nonsense\n").is_err());
        assert!(parse_inventory(&format!("{}  a\n", D.to_uppercase())).is_err());
        assert!(parse_inventory(&format!("{D}  ../escape\n")).is_err());
        assert!(parse_inventory(&format!("{D}  /abs\n")).is_err());
        assert!(parse_inventory(&format!("{D}  a//b\n")).is_err());
        assert!(parse_inventory(&format!("{D}  a\n{D}  a\n")).is_err());
    }

    fn statuses(o: &Outcome, id: &str) -> Vec<(Status, Option<&'static str>)> {
        o.checks
            .iter()
            .filter(|c| c.id == id)
            .map(|c| (c.status, c.failure_class))
            .collect()
    }

    #[test]
    fn write_then_check_inventory_round_trips_and_catches_a_flipped_byte() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("v")).unwrap();
        std::fs::write(dir.path().join("v/x.bin"), [1u8, 2, 3]).unwrap();
        std::fs::write(dir.path().join("top.txt"), "t").unwrap();
        assert_eq!(write_inventory(dir.path()).unwrap(), 2);
        let ok = recheck(dir.path(), None).unwrap();
        assert_eq!(statuses(&ok, "inventory"), vec![(Status::Pass, None)]);

        std::fs::write(dir.path().join("v/x.bin"), [1u8, 2, 2]).unwrap();
        let bad = recheck(dir.path(), None).unwrap();
        assert!(!bad.ok);
        assert!(bad.failure_classes().contains("inventory-digest-mismatch"));

        std::fs::write(dir.path().join("v/x.bin"), [1u8, 2, 3]).unwrap();
        std::fs::write(dir.path().join("v/extra"), "e").unwrap();
        let extra = recheck(dir.path(), None).unwrap();
        assert!(extra.failure_classes().contains("inventory-unlisted-file"));

        std::fs::remove_file(dir.path().join("v/extra")).unwrap();
        std::fs::remove_file(dir.path().join("top.txt")).unwrap();
        let missing = recheck(dir.path(), None).unwrap();
        assert!(missing.failure_classes().contains("inventory-missing-file"));
    }

    #[test]
    fn missing_run_json_is_a_required_file_failure() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("report.json"), "{}\n").unwrap();
        let o = recheck(dir.path(), None).unwrap();
        assert!(!o.ok);
        assert!(o.failure_classes().contains("required-file-missing"));
        assert!(o.to_text().contains("NOT-CHECKED"));
    }

    #[test]
    fn empty_suite_fails_rather_than_passing_vacuously() {
        let dir = tempfile::tempdir().unwrap();
        let o = recheck(dir.path(), None).unwrap();
        assert!(!o.ok);
        assert!(o.failure_classes().contains("no-verdicts"));
    }

    #[test]
    fn link_reports_unrecorded_as_not_checked() {
        let mut c = Checks::default();
        link(&mut c, "l".into(), None, "w", "t", Some(D), "not recorded");
        link(&mut c, "l".into(), Some(D), "w", "t", Some(D), "x");
        link(&mut c, "l".into(), Some(D), "w", "t", None, "x");
        link(
            &mut c,
            "l".into(),
            Some(D),
            "w",
            "t",
            Some(D.replace('a', "b").as_str()),
            "x",
        );
        let got: Vec<_> = c.0.iter().map(|k| (k.status, k.failure_class)).collect();
        assert_eq!(
            got,
            vec![
                (Status::NotChecked, None),
                (Status::Pass, None),
                (Status::Fail, Some("chain-target-missing")),
                (Status::Fail, Some("chain-mismatch")),
            ]
        );
    }

    #[test]
    fn first_difference_names_the_line() {
        let d = first_difference(b"a\nb\nc\n", "a\nX\nc\n");
        assert!(d.contains("line 2"), "{d}");
    }
}

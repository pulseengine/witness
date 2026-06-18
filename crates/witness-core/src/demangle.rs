//! Function-symbol demangling for human-readable branch attribution.
//!
//! witness-core stores the raw name-section symbol as ground truth
//! ([`crate::instrument::BranchEntry::function_name`]) for attestation
//! stability — demangler output can change across `rustc-demangle`
//! versions, so the canonical evidence body keeps the original symbol.
//! This helper derives the readable *display* form
//! ([`crate::instrument::BranchEntry::function_display`]).
//!
//! Demangling lives here, once (DEC-038), so every emitter —
//! `report` JSON, LCOV, rivet-evidence, the predicate Statement, and
//! the viz dashboard — shares one implementation instead of each
//! re-demangling the raw symbol (the duplication the toolchain avoids).
//!
//! Closes part of REQ-055 / FEAT-035.

/// Demangle a Rust or C++ mangled symbol into a human-readable name.
///
/// The scheme is detected, not guessed: a symbol is Rust iff
/// [`rustc_demangle::try_demangle`] accepts it (covers both the legacy
/// `_ZN..E` mangling and the v0 `_R..` mangling), C++ iff
/// [`cpp_demangle`] parses it, otherwise it is returned unchanged.
///
/// Rust symbols use the `{:#}` alternate form so the trailing
/// `::h<hash>` disambiguator is dropped — matching what witness-viz
/// already rendered, e.g.
/// `_ZN17verdict_leap_year12is_leap_year17h..E` →
/// `verdict_leap_year::is_leap_year`.
pub fn demangle(raw: &str) -> String {
    // Arm 1 — Rust. `try_demangle` returns Err on a non-Rust input, so
    // (unlike the infallible `demangle`) it actually decides the scheme.
    if let Ok(sym) = rustc_demangle::try_demangle(raw) {
        return format!("{sym:#}");
    }
    // Arm 2 — C++ (Itanium ABI). `Symbol::new` parses the mangling;
    // `demangle` renders it. cpp_demangle 0.5 dropped the options arg
    // (the renderer takes no DemangleOptions) — call it bare.
    if let Ok(sym) = cpp_demangle::Symbol::new(raw)
        && let Ok(s) = sym.demangle()
    {
        return s;
    }
    // Arm 3 — not a recognised mangling: pass through unchanged.
    raw.to_string()
}

/// Demangle an optional raw symbol, preserving `None`.
///
/// Convenience for the attribution site, where `function_name` is
/// `Option<String>` (absent when the module carries no name section).
pub fn demangle_opt(raw: Option<&str>) -> Option<String> {
    raw.map(demangle)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The motivating case: the leap-year fixture's Rust symbol, exactly
    /// as it survives `meld fuse --preserve-names` into the fused core.
    #[test]
    fn rust_legacy_symbol_demangles_and_drops_hash() {
        let raw = "_ZN17verdict_leap_year12is_leap_year17hdd79b6616066b4acE";
        assert_eq!(demangle(raw), "verdict_leap_year::is_leap_year");
    }

    /// Rust v0 mangling (`_R..`) is also recognised by rustc-demangle.
    #[test]
    fn rust_v0_symbol_demangles() {
        // `_RNvCs..` form; assert it changed and lost the leading `_R`.
        let raw = "_RNvNtCs1234_7mycrate3foo3bar";
        let out = demangle(raw);
        assert!(out.contains("bar"), "expected demangled v0 name, got {out}");
        assert!(!out.starts_with("_R"), "v0 symbol left mangled: {out}");
    }

    /// C++ Itanium mangling demangles via cpp_demangle (arm 2).
    #[test]
    fn cpp_symbol_demangles() {
        // `_Z3foov` == `foo()`.
        assert_eq!(demangle("_Z3foov"), "foo()");
    }

    /// A name that matches no scheme passes through unchanged (arm 3).
    #[test]
    fn plain_name_passes_through() {
        assert_eq!(demangle("run_row_0"), "run_row_0");
        assert_eq!(demangle("memory"), "memory");
        assert_eq!(demangle(""), "");
    }

    /// `demangle_opt` preserves `None` and maps `Some`.
    #[test]
    fn opt_preserves_none_and_maps_some() {
        assert_eq!(demangle_opt(None), None);
        assert_eq!(demangle_opt(Some("_Z3foov")).as_deref(), Some("foo()"));
    }

    /// MC/DC truth table for the scheme-selection decision (step 5 of the
    /// feature loop — witness the decision, don't trust a percentage).
    ///
    /// Conditions: A = `try_demangle` Ok (Rust), B = `Symbol::new` Ok
    /// (C++ parse), C = cpp `demangle` Ok (C++ render).
    ///
    /// | # | A | B | C | outcome      | covered |
    /// |---|---|---|---|--------------|---------|
    /// | 1 | T | – | – | Rust         | yes     |
    /// | 2 | F | T | T | C++          | yes     |
    /// | 3 | F | F | – | passthrough  | yes     |
    /// | 4 | F | T | F | passthrough* | GAP     |
    ///
    /// Row 4 is a defensive fall-through: a symbol cpp_demangle *parses*
    /// but fails to *render* (e.g. recursion-limit). Its observable
    /// outcome is identical to row 3 (the raw string passes through), and
    /// no simple deterministic input reaches it — so it is documented as
    /// an honest gap row rather than covered by a brittle fixture. A and
    /// B each have a unique-cause pair among rows 1–3; C's independence
    /// pair (rows 2 vs 4) is the gap.
    #[test]
    fn mcdc_decision_arms_reachable_rows() {
        // Row 1 — A=T.
        assert_eq!(
            demangle("_ZN17verdict_leap_year12is_leap_year17hdd79b6616066b4acE"),
            "verdict_leap_year::is_leap_year"
        );
        // Row 2 — A=F, B=T, C=T.
        assert_eq!(demangle("_Z3foov"), "foo()");
        // Row 3 — A=F, B=F. Passthrough; output equals input.
        assert_eq!(demangle("run_row_0"), "run_row_0");
    }
}

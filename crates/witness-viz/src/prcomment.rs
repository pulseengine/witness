//! PR-comment MC/DC delta renderer (DEC-032, REQ-049).
//!
//! Pure function: takes two loaded report sets (base + head) and
//! returns a Markdown string suitable for `gh pr comment --body-file`.
//! No I/O, no axum — the binary loads the verdicts and pipes the
//! result to stdout or `--out`.
//!
//! Matching is by stable identity keys (DEC-032): verdicts by name,
//! decisions by `id`, conditions by `index`. Anything present on only
//! one side is reported as added/removed, never fuzzily diffed.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use crate::data::{McdcReport, VerdictBundle};

/// Render the Markdown MC/DC delta between `base` and `head`.
pub fn render_pr_comment(base: &[VerdictBundle], head: &[VerdictBundle]) -> String {
    let base_by: BTreeMap<&str, &McdcReport> =
        base.iter().map(|v| (v.name.as_str(), &v.report)).collect();
    let head_by: BTreeMap<&str, &McdcReport> =
        head.iter().map(|v| (v.name.as_str(), &v.report)).collect();

    let mut names: BTreeSet<&str> = BTreeSet::new();
    names.extend(base_by.keys().copied());
    names.extend(head_by.keys().copied());

    let mut md = String::new();
    md.push_str("## MC/DC coverage delta\n\n");

    if names.is_empty() {
        md.push_str("_No verdicts on either side._\n");
        return md;
    }

    md.push_str("| verdict | decisions | full MC/DC | proved | gap | dead |\n");
    md.push_str("|---|---|---|---|---|---|\n");

    // TOTAL accumulators (base, head) per column.
    let mut t = Totals::default();

    for name in &names {
        let b = base_by.get(name).copied();
        let h = head_by.get(name).copied();
        let (bo, ho) = (b.map(|r| r.overall), h.map(|r| r.overall));

        // Marker for added / removed verdicts.
        let label = match (b, h) {
            (None, Some(_)) => format!("`{name}` 🆕"),
            (Some(_), None) => format!("`{name}` ❌removed"),
            _ => format!("`{name}`"),
        };

        let bd = bo.map_or(0, |o| o.decisions_total);
        let hd = ho.map_or(0, |o| o.decisions_total);
        let bf = bo.map_or(0, |o| o.decisions_full_mcdc);
        let hf = ho.map_or(0, |o| o.decisions_full_mcdc);
        let bp = bo.map_or(0, |o| o.conditions_proved);
        let hp = ho.map_or(0, |o| o.conditions_proved);
        let bg = bo.map_or(0, |o| o.conditions_gap);
        let hg = ho.map_or(0, |o| o.conditions_gap);
        let bde = bo.map_or(0, |o| o.conditions_dead);
        let hde = ho.map_or(0, |o| o.conditions_dead);

        let _ = writeln!(
            md,
            "| {label} | {} | {} | {} | {} | {} |",
            delta_cell(bd, hd),
            delta_cell(bf, hf),
            delta_cell(bp, hp),
            delta_cell(bg, hg),
            delta_cell(bde, hde),
        );

        t.add(bd, hd, bf, hf, bp, hp, bg, hg, bde, hde);
    }

    let _ = writeln!(
        md,
        "| **TOTAL** | {} | {} | {} | {} | {} |",
        delta_cell(t.bd, t.hd),
        delta_cell(t.bf, t.hf),
        delta_cell(t.bp, t.hp),
        delta_cell(t.bg, t.hg),
        delta_cell(t.bde, t.hde),
    );
    md.push('\n');

    // Per-condition status transitions.
    let mut regressions = Vec::new();
    let mut improvements = Vec::new();
    let mut other = Vec::new();

    for name in &names {
        let (Some(b), Some(h)) = (base_by.get(name).copied(), head_by.get(name).copied()) else {
            continue; // added/removed verdicts already flagged in the table
        };
        collect_transitions(
            name,
            b,
            h,
            &mut regressions,
            &mut improvements,
            &mut other,
        );
    }

    let section = |md: &mut String, title: &str, items: &[String]| {
        if items.is_empty() {
            return;
        }
        let _ = writeln!(md, "### {title}");
        for it in items {
            let _ = writeln!(md, "- {it}");
        }
        md.push('\n');
    };

    if regressions.is_empty() && improvements.is_empty() && other.is_empty() {
        md.push_str("_No per-condition status changes._\n");
    } else {
        // Regressions first — that's what blocks a PR.
        section(&mut md, "⚠️ Regressions (proved → gap/dead)", &regressions);
        section(&mut md, "✅ Improvements (gap/dead → proved)", &improvements);
        section(&mut md, "Other transitions (gap ↔ dead)", &other);
    }

    md
}

#[derive(Default)]
struct Totals {
    bd: u32,
    hd: u32,
    bf: u32,
    hf: u32,
    bp: u32,
    hp: u32,
    bg: u32,
    hg: u32,
    bde: u32,
    hde: u32,
}

impl Totals {
    #[allow(clippy::too_many_arguments)]
    fn add(
        &mut self,
        bd: u32,
        hd: u32,
        bf: u32,
        hf: u32,
        bp: u32,
        hp: u32,
        bg: u32,
        hg: u32,
        bde: u32,
        hde: u32,
    ) {
        self.bd = self.bd.saturating_add(bd);
        self.hd = self.hd.saturating_add(hd);
        self.bf = self.bf.saturating_add(bf);
        self.hf = self.hf.saturating_add(hf);
        self.bp = self.bp.saturating_add(bp);
        self.hp = self.hp.saturating_add(hp);
        self.bg = self.bg.saturating_add(bg);
        self.hg = self.hg.saturating_add(hg);
        self.bde = self.bde.saturating_add(bde);
        self.hde = self.hde.saturating_add(hde);
    }
}

/// Walk matched decisions (by id) and conditions (by index), bucket
/// each status change into regressions / improvements / other.
fn collect_transitions(
    verdict: &str,
    base: &McdcReport,
    head: &McdcReport,
    regressions: &mut Vec<String>,
    improvements: &mut Vec<String>,
    other: &mut Vec<String>,
) {
    let base_dec: BTreeMap<u32, &crate::data::DecisionReport> =
        base.decisions.iter().map(|d| (d.id, d)).collect();
    for hd in &head.decisions {
        let Some(bd) = base_dec.get(&hd.id) else {
            continue; // new decision — counted in the table, not a transition
        };
        let base_cond: BTreeMap<u32, &str> = bd
            .conditions
            .iter()
            .map(|c| (c.index, c.status.as_str()))
            .collect();
        for hc in &hd.conditions {
            let Some(&bs) = base_cond.get(&hc.index) else {
                continue;
            };
            let hs = hc.status.as_str();
            if bs == hs {
                continue;
            }
            let loc = format!(
                "`{verdict}` decision #{id} c{ci} (`{src}:{line}`): {bs} → {hs}",
                id = hd.id,
                ci = hc.index,
                src = hd.source_file,
                line = hd.source_line,
                bs = bs,
                hs = hs,
            );
            match (bs == "proved", hs == "proved") {
                (true, false) => regressions.push(loc),
                (false, true) => improvements.push(loc),
                _ => other.push(loc),
            }
        }
    }
}

/// Render a single before→after table cell. Unchanged values show a
/// bare number; changes show `before → after (±N)`.
fn delta_cell(before: u32, after: u32) -> String {
    if before == after {
        return after.to_string();
    }
    let arrow = if after > before {
        format!("+{}", after - before)
    } else {
        format!("-{}", before - after)
    };
    format!("{before} → {after} ({arrow})")
}

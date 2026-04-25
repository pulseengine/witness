# safety_envelope — V-model traceability

## Requirement chain

- **REQ-027** Truth-table emission
- **REQ-028** Independent-effect citation
- **REQ-030** Verdict suite
- **REQ-015** No artificial condition-count cap (this verdict's reason for being)

## Design decisions

- **DEC-007** Reject the LLVM 6-condition cap (this verdict is the proof)
- **DEC-013** Trace-buffer primitive (no encoder constraint)
- **DEC-014** Short-circuit preserved
- **DEC-016** Verdict suite composition

## Conditions

5 boolean expressions on a `Telemetry` struct:
c1 = temp range, c2 = pressure range, c3 = rpm range, c4 = fault-free, c5 = mode active.

## Test rows

6 rows (`run_row_0` through `run_row_5`). The optimal N+1 = 6 pattern
for a 5-condition AND chain under masking MC/DC.

## Evidence

`compliance/verdict-evidence/safety_envelope/`.

## Why this verdict exists

This is the **scaling proof**. Three reasons:

1. **REQ-015 verification.** The "no artificial condition-count cap"
   requirement is meaningful only if the suite contains a decision
   that demonstrates it. 5 conditions is below the LLVM 6-cap but
   safety_envelope's structure trivially extends to 7 or 8 conditions
   if the v0.7 scaling work needs further proof.
2. **Real-world shape.** Safety envelopes (automotive E/E, aerospace,
   industrial control interlocks) are almost always long AND chains
   of range and state checks. This verdict isn't synthetic in the
   leap_year sense — it's a structural model of working safety
   software.
3. **Optimal-row-pattern test.** N+1 rows (one per becoming-first-F
   transition, plus one all-T) is the minimal MC/DC test set for
   N-cond AND. The reporter's gap-closure recommendation must
   reproduce this pattern when given an incomplete suite — this
   verdict is the test oracle for that algorithm.

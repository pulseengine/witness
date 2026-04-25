# Loom + meld upstream issue drafts

Witness depends on stable `(byte_offset → source file/line)` mapping
to do MC/DC reconstruction. Today witness consumes pre-loom /
pre-meld Wasm only. For v0.6+ to measure post-loom (optimised) and
post-meld (fused) Wasm without losing the source-level decision
binding, both tools need to emit a translation map alongside their
output.

These are issue bodies to file at:
- https://github.com/pulseengine/loom/issues/new
- https://github.com/pulseengine/meld/issues/new

Filed manually by the maintainer. Drafts below.

---

## Loom — DWARF + address-translation forwarding

**Title:** `Preserve DWARF and emit a byte-offset translation map for downstream coverage tooling`

**Body:**

Witness (https://github.com/pulseengine/witness) measures branch
coverage on Wasm bytecode by correlating instruction byte offsets to
source `(file, line)` via DWARF `.debug_line`. For v0.5 witness
operates on pre-loom modules. The next-version goal is to measure the
*post-loom-optimised* module — the bytes that actually ship — without
losing the source-level decision binding loom's input has.

### What we need

When loom rewrites a module via Z3-validated optimisation, witness
needs **two** kinds of evidence to keep its coverage claims sound:

1. **Forwarded `.debug_*` custom sections.** If loom drops DWARF, all
   witness sees on the post-loom module is unmapped instruction
   offsets. The minimum is to preserve the original sections; a
   higher fidelity would be re-emitting them with the post-loom
   offsets.
2. **An address-translation map.** The bytes of pre-loom function `f`
   at offset `O_pre` correspond to post-loom function `f'` at offset
   `O_post`. This is a per-(function, offset) map. A reasonable on-
   disk shape:

   ```json
   {
     "schema": "https://pulseengine.eu/loom-translation/v1",
     "loom_version": "0.x.y",
     "input_module": "<sha256>",
     "output_module": "<sha256>",
     "function_map": [
       { "input_index": 12, "output_index": 12 }
     ],
     "offset_map": [
       { "input_function": 12, "input_offset": 4, "output_function": 12, "output_offset": 0 }
     ]
   }
   ```

   The exact shape can iterate. The load-bearing property: witness
   can take a post-loom offset and resolve it back to a pre-loom
   offset, then resolve that to a `(file, line)` via the
   pre-loom DWARF.

### Why "preserve DWARF" alone is not enough

LLVM-style "preserve DWARF through optimisation" forwards what LLVM
emitted, but only at the instruction level — and only for instructions
that survive the optimisation. Optimisations that *eliminate* code
paths (constant-folded `br_if`, dead-code elimination, loop fusion)
break the assumption that every post-loom instruction has a pre-loom
preimage. Witness needs the explicit map because a *missing* offset
in the map is the signal "this code was eliminated during
optimisation" — which is itself meaningful coverage evidence.

### Reference implementations

- LLVM's `llc -dwarf-version=5` documents the address-stability
  guarantee for unoptimised builds; the optimised case is
  best-effort.
- CompCert's translation-validation produces a per-input
  correctness certificate; loom's Z3 TV is the structural analog.

### Witness-side commitment

When loom ships `(forwarded DWARF + offset map)`, witness v0.6 will:
- Read the offset map alongside the post-loom Wasm.
- Use it to project post-loom offsets back to pre-loom DWARF
  `(file, line)`.
- Emit MC/DC `Decision`s that are sound relative to **source-level**
  decisions, not just post-loom decisions.

### Acceptance criteria for this issue

- [ ] DWARF `.debug_*` custom sections are preserved in loom's output
      modules (verifiable via `wasm-tools dump`).
- [ ] An offset translation map is written to a sidecar file (or
      embedded as a custom section) at a stable schema URL.
- [ ] At least one round-trip test: a pre-loom module's `br_if`
      branch resolves to the same `(file, line)` via the post-loom
      module + map.

---

## Meld — DWARF + per-input-module byte-offset map for fused components

**Title:** `Preserve DWARF and emit per-input-module offset maps for fused components`

**Body:**

Meld fuses N Wasm core modules into one component. Witness wants to
measure coverage on the fused output and untangle which bytes came
from which input module so source-level claims survive fusion.

### What we need

When meld fuses inputs `M_1, ..., M_N` into output `F`:

1. **Forward each input's DWARF sections** under
   `.debug_info_<i>` (or any disambiguating naming). Today meld may
   drop DWARF entirely or merge it ambiguously; we need the per-input
   distinction.
2. **A per-input offset map** indicating, for each byte offset in
   `F`, which input module's offset it came from:

   ```json
   {
     "schema": "https://pulseengine.eu/meld-fusion-map/v1",
     "meld_version": "0.x.y",
     "output_module": "<sha256>",
     "inputs": [
       { "name": "auth.wasm", "sha256": "...", "input_index": 0 },
       { "name": "logger.wasm", "sha256": "...", "input_index": 1 }
     ],
     "fusion_offset_map": [
       { "output_function": 7, "output_offset": 0,
         "input_index": 1, "input_function": 3, "input_offset": 12 }
     ]
   }
   ```

### Why per-input is load-bearing

Witness's manifest assigns a globally-unique `branch_id` per branch.
After fusion, branches from different input modules collide in
`(function_index, instr_index)` space. Without the per-input
disambiguation, witness cannot tell two distinct branches apart.

### Acceptance criteria

- [ ] Per-input DWARF preserved (under disambiguating section names).
- [ ] Fusion-offset map emitted at a stable schema URL.
- [ ] Round-trip test: fuse two modules, instrument the fused output
      with witness, verify each branch traces back to its input
      module.

---

## Why we are filing these now

Witness's v0.4 paper documents the post-loom / post-meld plan
(`docs/paper/v0.2-mcdc-wasm.md` §11). The witness side of that work
is gated by these upstream features. Filing the issues now lets the
loom and meld maintainers prioritise — or push back if the design
doesn't match their constraints.

Witness v0.5 ships without these dependencies. Witness v0.6 depends
on at least the DWARF-preservation half; the offset-map half is the
v0.7+ ambition.

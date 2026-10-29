# Section Refit development

`Refit=0.01` and `Simplify=Refit RefitTolerance=0.01` fit copies of real
profiles before point-end poles are added. Rebuild and Refit are mutually
exclusive, and authored rail stations are retained.

Compatibility remains incomplete at comparison epsilon `1e-7`:

| Cases | Constructed | Complete surface images proved |
| --- | ---: | ---: |
| Base SDK, command and controlled sweeps | 86/86 | 83/86 |
| Re-entered C1 profiles | 12/12 | 8/12 |
| Combined | 98/98 | 91/98 |

The [latest full replay](refit-separation-validation.json) retains seven failing
surface assertions on both backends. Exact projected distance bounds certify
these geometric gaps, charging stored-coordinate and weight rounding. A proved
separation skips further correspondence attempts; numerical witnesses alone
cannot establish either agreement or disagreement.

The [direct fitting replay](refit-section-api-validation.json) constructs all
16 original/C1 and single/paired contexts. Eight match complete native curves;
eight have certified geometric gaps, including two additional original paired
profiles. All assertions remain active. The [section-only oracle API](section-fitting-api.md)
exposes this preparation without rail frames or surface construction. RPC checks
cover coarse and disabled fitting, authored parameters and input preservation on
both backends; the fine cases retain their separate required comparison gate.

[Large-tolerance fitting](refit-large-tolerances.md) admits certified proposals
at and above endpoint-chord lengths and retains closed loops. Its independent
profiles and endpoint-opposition controls add required complete-image gates;
their remaining differences are recorded separately from the historical replay.

Rust currently uses two independently certified passes for a single smooth
profile. Paired fitting reconstructs independently fitted curves at shared
stations and requires a fresh complete original-source bound. Straight and
polygon profiles keep their own path. Nominal tolerance is not a final
source-distance guarantee across successive passes: a native 0.001 single fit
can lie more than 0.0015 from a source point.
A paired native fit also contains a point more than `6.26e-7` from its source
at tolerance `1e-6`, disproving a universal half-tolerance source reserve.

Complete image certificates gate all fitting proposals. Target-knot witnesses
and tangent frames help qualify reconstruction across independent parameter
clocks. Single-profile assemblies retry these proposals when their original
fitting stations cannot prove the retimed curve, keeping the same complete
source bound and tolerance. Correspondence proposals refine to at most 1024
intervals; arithmetic and distance limits remain fixed. Whole-surface proofs
include U-degree elevation rounding and every profile row. Matching only end
rows cannot prove agreement.
The fitting tests may use observed cuts to isolate piece construction; production
never reads native cuts or control nets.

[Public fitting diagnostics](refit-cubic-validation.json) reproduce 480 native
continuity queries and certify 96 full or trimmed inputs within `1.2e-14`.
Public `MakeCompatible` outputs exactly reproduce 32 complete Sweep boundary
definitions. Reconstructed C1 inputs are independent test curves, not claimed
native intermediates. Public cubic-Bezier fitting is a separate diagnostic API.

[Representation diagnostics](refit-representation-validation.json) prove 12
equivalent inputs at `1e-12`; inserting knots nevertheless changes two native
sweep images beyond `1e-7`. Public closest-point queries agree within `1e-7`.
The [clock correction](refit-clock-validation.json) resolves 16 multi-span
batches and adds 450 precision queries. Numerical NURBS batches share request
normalization; fitting uses public request `0.01`, effective tolerance `1e-4`.

UI regressions cover circular sections, both tolerance getters, `Simplify`,
explicit and automatic stations, cancellation, and local/model undo and redo.
Broader command/UI qualification remains outstanding. Historical inventories
still describe `main` before this branch and must be refreshed before merging.

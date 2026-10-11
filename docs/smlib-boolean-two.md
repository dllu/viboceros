# Native two-object Boolean choices

Linux builds with `--features native-smlib` prepare closed Boolean2Objects
operands directly in usd-brep. Open planar inputs retain the existing exact
sheet arrangement. Native geometry failures propagate before preview or model
changes; a closed native failure does not start a second geometry implementation.

The adapter imports and validates the two original solids once, then performs
Union, Intersection and both Differences on copied operands. Kernel boundary
contact and empty common material reject disjoint, strictly enclosed and point
contact pairs. Empty differences in both directions reject equal solids,
preserving the existing closed-pair policy. Topology cleanup is disabled to
preserve imprinted seams.
Inverse intersection reuses the two prepared Difference branches in B−A, A−B
order. All five choices remain cached across preview wraparound and acceptance.

Every piece retains the first source's identity/attributes policy. Geometry
user text survives a single material body in each branch; a disconnected branch
clears it. Existing preview cancellation, source freshness, DeleteInput and
transactional Undo/Redo flow remain shared with planar preparation. No scalar
volume threshold discards small common material.

The original eleven public box captures still qualify cycle results, geometry,
metadata and history. Eight new [cylinder/slab captures](../tools/rhino_oracle/observations/boolean_two_curved.json)
cover all five choices, wraparound, retained inputs and cancellation, using real
bounded viewport clicks under private Xvfb. They confirm first-source ownership
and separate text retention for the three inverse-intersection pieces.

Independent Rust volume and area queries qualify the five curved material sets
against analytic values at 1e-8. Spatial edge and lifted-trim samples qualify
their analytic cylinder/slab boundaries at the same epsilon. Command output
geometry must match those prepared candidates. Attributes, text, closure and
history remain exact comparisons; body order and source-ID association
differences are gated separately below. Recorded Rhino scalar volume/area queries
use a separate 2e-6 comparison bound because their captured cylinder integrals
differ from analytic values. This does not prove whole-boundary equivalence or
general curved command parity.

Native edge/face partition and body order differences are explicitly gated by the
[representation ledger](smlib-boolean-two-partitions.json). Broader compounds,
tangencies, open curved sheets and performance parity remain under qualification.

Two additional [public box-split captures](../tools/rhino_oracle/observations/boolean_two_split.json)
qualify disconnected branch order, first-source identity and cleared text in
Difference and inverse intersection. Rhino's cylinder split returns the upper
body first, while its box split returns the lower body first. Native staging
keeps kernel body order: the box replay matches exactly, but the cylinder's
Difference assigns the first source's ID to the lower body. That ID-association
difference and the inverse body order are retained in the ledger. All curved
bodies are paired by qualified mass/centroid for metadata comparison; a universal
body reversal is not supported by the evidence.
[Capture and replay provenance](smlib-boolean-two-provenance.json) records
artifact hashes, kernel revision, checks and these limits.

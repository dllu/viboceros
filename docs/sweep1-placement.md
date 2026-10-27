# Sweep1 section placement

[Sweep1](sweep1.md) · [Rhino oracle](oracle.md)

The [validation record](sweep1-placement-validation.json) binds the refactor,
original command source, capture inputs, archived producers and regression checks.

Automatic rail/profile picking is still pending. The current command requires
explicit `Parameters=`. Option parsing and regression tests now live in
`crates/viboceros-command/src/sweep/`, separate from construction and admission.

Two immutable captures retain seven independently authored inputs through both
the public Rhino SDK and actual `Sweep1` command: start, interior and end
profiles, an offset profile, a sloped profile, and two interior profiles in both
selection orders. The [placement capture](../tools/rhino_oracle/observations/sweep1_section_placement.json)
has 14 results; the [definition capture](../tools/rhino_oracle/observations/sweep1_section_definitions.json)
adds 14 complete NURBS surface records and wider closest-point queries.
Each capture records its request hash and archived producer bytes.

For the straight rail from Z=0 to Z=5, the interior profile at Z=2 produces
control bounds Z=2 to Z=5. An offset profile at that height retains its offset.
The sloped profile has bounds Z=1 to Z=8 because its own height remains part
of the swept geometry.

The SDK and command differ in two cases. A single profile at the rail end
produces SDK control bounds Z=5 to Z=10; the command instead sweeps back to Z=0.
For profiles selected at Z=4 then Z=1, the command retains bounds Z=1 to Z=4,
while the SDK result has a control hull reaching Z=0 and Z=5. These observations
require explicit handling of rail direction and profile order in the future
interactive workflow. They do not establish rules for all curved rails.
Both reversed-order results retain the first section at Z=4 and the last at Z=1.
Sorting profiles silently into increasing original rail parameters would therefore
lose the measured traversal.

The fixture's `parameters` field is consumed by Viboceros's existing explicit
station harness; Rhino receives the source curves and determines placement itself.
Those authored numbers are not measured Rhino station outputs. The new captures
are investigation inputs, rather than a claim that automatic placement is
implemented or that sampled closest points certify equal surfaces.

```sh
python3 -m unittest tools.rhino_oracle.test_sweep1_section_placement
cargo test -p viboceros-command --lib sweep::tests --frozen
```

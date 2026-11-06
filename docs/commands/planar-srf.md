# PlanarSrf

Select planar boundary curves or connected open pieces, then enter:

```text
PlanarSrf
PlanarSurface DeleteInput=Yes
```

Starting the command without preselection opens a curve selection prompt with
a `DeleteInput` option. Both builds create exact trimmed planar B-reps. Nested
loops alternate between holes and independent islands. Overlapping loops use the measured Rhino seam
point parity, followed by whole-loop hole admission. Moving a closed curve's
seam can therefore change which overlapping boundary becomes a hole. Curves on
other planes cannot affect that parity. A curve touching its prospective outer
boundary remains independent; sibling holes can touch each other.

Repeated identical encodings coalesce before parity classification. The captured
polygon cases retain distinct reversed or shifted-seam encodings, including
separate coincident hole loops. Equivalent full-circle encodings coalesce. These
rules preserve the selected source curves and their parameterization. Coincident
polygon walks contribute one geometric mesh hole while their original B-rep
loops remain intact. Reported model area can count repeated loops, as Rhino's
area calculation does. [Crossing holes](planar-crossings.md) also retain separate
model loops while their displayed mesh excludes the holes' union.

Outputs have fresh attributes on the current layer, remain unselected and
ungrouped, and share one Undo/Redo step. Retained inputs keep their attributes,
groups and selection. `DeleteInput=Yes` deletes source pieces contributing to
accepted profiles, including coalesced duplicates and partial overlaps; unused
tails remain. It is an application extension; Rhino PlanarSrf retains its inputs.
Connected open pieces use the existing batch curve joiner. Incomplete chains
without a recovered planar loop and nonplanar profiles are skipped.
[Open-chain behavior and qualification](planar-chains.md) include tolerance
closure, branches and recovered filled regions. At most 128 closed planar
boundaries participate.

The shared planner computes source-plane containment and checks each proposed
hole against its entire outer boundary. Contact proposals require a checked
spatial witness before the existing general intersection search. Native builds
admit every kernel profile first, then ask usd-brep to construct each planned
region. Rejected construction or changed loop multiplicity uses the existing
exact Rust constructor. [Bridge details](../smlib-planar-surfaces.md).

The [validation record](../planar-regions-validation.json) covers 105 distinct
native source scenes and 109 application and output 3DM cases in each build,
with additional diagnostic and confirmation captures. It checks original
boundary membership, region counts, area, source definitions and history.
Source definitions use the public homogeneous-to-Euclidean control chart at
absolute epsilon `1e-12`; stored Rust source objects remain exactly unchanged.
Precise areas use absolute epsilon `1e-9` plus relative `1e-12`. Boundary identity
uses spatially checked samples at `1e-7`. Raw captures retain full used B-rep
definitions. Complete native B-rep parameterization, output/loop birth order,
continuous curve-contact completeness and comparative performance remain
unqualified. Crossing sibling holes have
[additional qualification](planar-crossings.md); arbitrary self-intersections
and further coincident curve encodings need broader qualification.

```sh
cargo test -p viboceros-oracle --test planar_regions --test planar_regions_archive
cargo test -p viboceros planar_regions
python3 -m unittest tools.rhino_oracle.test_planar_regions
```

Licensed captures use private Xvfb sessions through
`tools/rhino_oracle/run_headless.sh`.
Reference: [Rhino PlanarSrf](https://docs.mcneel.com/rhino/8/help/en-us/commands/planarsrf.htm).

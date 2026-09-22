# Scale-independent Join orientation

[Join commands](commands/join.md) · [Boundary assembly](brep-edge-joining.md)

Automatic B-rep joining now uses [exact spatial orientation](solid-orientation.md)
to orient a newly closed connected shell. The former unconditional signed-volume
test could leave a tiny shell inward when its volume rounded to zero, or fail a
valid large join because the volume calculation overflowed.

A box with side lengths `s`, `2s`, `4s` has volume `8s³`. At `s=2^-360`, this is
`2^-1077`, below binary64's smallest nonzero value; at `s=2^360`, it is `2^1083`,
beyond its largest finite value. The coordinates, surface areas, topology, and
orientation of both boxes remain well-defined. An orientation decision must not
require representing their volume as a scalar.

## Policy and tests

An exact inward witness reverses the newly joined component as a whole; an
outward witness or still-open component preserves its sense. Each output is
already one edge-connected component. An `Unknown` query retains the numerical
volume fallback for unsupported curved/trimmed representations. That fallback
has no rigorous integration-error certificate and can still fail at extreme
scales. Neither path proves global non-self-intersection or classifies cavities.
Untouched closed inputs and explicit `try_join_edge_pairs` assembly are unchanged.

The regression failed before the fix with an inward result at `2^-360` despite
consistent closed topology. Native kernel tests cover three scales, four initial
face-sense masks, both source orders, immutable inputs, and exact preservation of
underlying surfaces. Another 48 Join/JoinCopy workflows check both selection
modes, global reversal, source order, copied originals, attributes, groups,
selection, and atomic undo/redo. These use contacting pick sequences so every
command-first input participates.

## Shared-input command evidence

The [source-only generator](../tools/rhino_oracle/references/join_orientation.py)
produces three 32-case matrices. Each varies face-sense mask, table order,
Join/JoinCopy, and pre/postselection. The recorded command-first order puts an
opposite face second, which is skipped; those outputs are open five-face shells.
Preselection joins all six faces into a closed solid.

Two completed owned private-Xvfb sessions retain
[unit-scale records](../tools/rhino_oracle/observations/join_orientation.json) and
[large-scale records](../tools/rhino_oracle/observations/join_orientation_large.json).
Both engines consume the same native-exported 3DM inputs. Export verifies the
native round trip, and the public command probe checks that document insertion
preserves each source. EndCommand snapshots include object identity/attributes,
groups and selection. Only owned objects are deleted; prior document state is
restored.

The opt-in `definition_only` Join probe records every spatial/UV control, knot,
domain, weight, tolerance, topology field and face sense, plus closed/solid and
orientation queries. It computes no discarded samples or mass integrals; ordinary
Join probes keep their previous sampled/integral schema. Interchange validation
still checks source definitions and samples. Comparisons use **zero absolute
epsilon**, relative epsilon `1e-12`, so a fixed epsilon cannot hide a small model.

| Batch | Former native behavior | Updated native comparison |
| --- | --- | --- |
| Unit, 32 cases | 32 complete matches | 32 complete matches |
| `2^360`, 32 cases | 16 open matches; 16 closed-operation failures | 16 open matches; 16 explicit orientation differences; no execution failures |

At the large scale, Rhino reports `IsSolid=true` but `SolidOrientation=None` for
all 16 closed outputs. Eight retain an inward seed sense; eight face outward.
Native now completes all cases and normalizes each closed output outward.
Exact `Fraction` cross/dot predicates on the retained affine face coefficients
independently establish these senses. Every other raw geometry/document field
matches. The eight already-outward cases differ only in the reported orientation;
the eight inward cases also differ in all six topology face-sense flags.
These are retained compatibility differences, not comparisons rewritten to pass.

A [fresh 10-case session](../tools/rhino_oracle/observations/join_orientation_large_repeat.json)
reproduces eight complete large command records exactly, including four inward
outputs. Two additional uninserted shared-box queries, with opposite global
senses, also return `IsSolid=true` and orientation `None` in Rhino. Their exact
normals distinguish inward/outward, isolating the reported classification issue
from document insertion and command history. Native correctly distinguishes
the two senses; all other recorded query fields match.

## Native-only underflow evidence and interchange limits

OpenNURBS rejects the `2^-360` source faces before a shared-source Rhino command
can run: it classifies their underlying surfaces as closed, then rejects their
ordinary boundary trims as missing seams. No scaled proxy or widened tolerance
is substituted. The [initial source-attempt archive](../tools/rhino_oracle/observations/join_orientation_source_discovery.json)
retains all native failures; the same exploratory batch also used a globally
tiny construction tolerance that failed source validation at unit/large scales.
The completed captures use explicitly scale-specific construction/model tolerances.

The [tiny native baseline](../tools/rhino_oracle/observations/join_orientation_tiny_native_before.json)
retains eight inward closed outputs among its 32 successful native executions.
All are corrected now. A separate regression exactly rescales the updated tiny
definitions back to the unit witnesses, including spatial edge parameter lengths;
surface/UV parameters, weights, tolerances, topology and document fields are not
discarded. This is an analytic power-of-two equivariance check, **not** a Rhino
observation at the tiny scale.

Raw captures are retained without numeric rewriting, including negative zero.
[Provenance](join-orientation-provenance.json) records capture/source hashes;
the [complete comparison reports](../tools/rhino_oracle/observations/join_orientation_comparisons.json)
retain every before/after difference and failure. Timing fields are zero;
no speedup is claimed.

```sh
cargo test --release -p viboceros-geometry joined_solid_orientation
cargo test --release -p viboceros-command extreme_scale_join
cargo test --release -p viboceros-oracle join_command::tests::orientation
python3 -m unittest tools.rhino_oracle.test_join_orientation
python3 -m tools.rhino_oracle replay \
  tools/rhino_oracle/fixtures/join_orientation_large.json \
  --observations tools/rhino_oracle/observations/join_orientation_large.json \
  --absolute-epsilon 0 --relative-epsilon 1e-12
```

The large replay correctly exits nonzero for its 16 remaining differences.

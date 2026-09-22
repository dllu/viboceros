# Shared-source B-rep document admission

`document_brep` compares public document insertion and replacement on the same
native-exported 3dm B-rep. Unlike the earlier [Rhino-constructed orientation
audit](orientation-audit.md), this operation is accepted by both engines. It uses
the [solid-orientation probe's](solid-orientation.md) bounded source construction
and lossless topology/geometry export checks.

The source-only [matrix](../tools/rhino_oracle/fixtures/document_brep.json) contains
64 workflows: eight geometries, two global senses, two Rhino insertion overloads,
and selected/unselected replacement targets. Geometries include an ordinary box,
a negative-volume compound, a nested cavity, disjoint opposed equal-volume boxes,
coincident opposed boxes, an open box, inconsistent closed face incidence, and a
tetrahedron whose X extrema are corners. The generic `Objects.Add` overload and
`AddBrep` with kinky-face splitting disabled are recorded separately. Native
insertion has no implicit kink-splitting option.

Each workflow records:

- The caller-owned source and its globally reversed replacement input.
- Full inserted and replaced geometry, including every surface, edge, UV trim,
  domain, tolerance, and topology face-sense flag in the interchange schema.
- Orientation getters separately from geometric definitions and solid/closed flags.
- Object identity and count, name, group count, current-layer membership, and selection.
- Replacement success and source/replacement immutability.

Crucially, the replacement is reversed from the **original source**, not the
inserted object. Both engines receive identical replacement inputs even when
their insertion behavior differs. Native replacement uses `EveryReplacement`:
an explicit assignment remains an assignment if normalization eventually makes
the resulting geometry equal. This probe does not exercise Undo/Redo.

The adapters do not use signed volume to classify or normalize any output.
Definition-only recording performs no sampling or mass integration; the shared
artifact writer still verifies native reader definitions and samples before the
Rhino session starts. Timings are zero and are not performance measurements.

## Retained results

The [complete Rhino 8.32.26160.13001 response](../tools/rhino_oracle/observations/document_brep.json)
and [unfiltered native baseline comparison](../tools/rhino_oracle/observations/document_brep_before_report.json)
retain all 64 cases. The initial comparison has **16 matches, 48 differences,
and zero native failures**:

- All 16 open/inconsistently oriented cases agree completely; insertion preserves
  their sense and replacement accepts the reversed original source.
- Forty regular solid workflows differed because native insertion/replacement
  preserved inward inputs, while Rhino globally reversed them.
- Eight coincident opposed-shell workflows additionally retain native `Unknown`
  versus Rhino `Outward`/`Inward` getter differences. Their native orientation
  remains unresolved, not repaired with an ordering heuristic.

All shared input definitions match exactly. Every document metadata field and
every numeric output also matches exactly (maximum absolute numeric error zero).
Only the explicitly recorded orientation strings and whole-object face flags
differ. Every caller-owned input remains unchanged, and all replacement calls
succeed. Both insertion overloads produce the same geometric result in this
matrix, with selection preserved in both selected/unselected cases.

Exact rational cross/dot products independently check the retained box-face
normals. The outward-classified negative-volume compound has analytic signed
volume −56; the opposed equal-size compounds have volume zero. Neither signed
volume nor independent per-shell outward orientation describes the observed
whole-object normalization. The cavity retains opposite outer/inner senses.

[Provenance](document-brep-provenance.json) records source and shared artifact
hashes. Lossless JSON compaction was checked recursively, including each scalar
type and every binary64 bit (signed zero included). Replay tests explicitly
assert the remaining eight coincident-query gaps before checking every remaining
field; the original 40 normalization-only gaps remain in the baseline report.

## Document normalization and import

`document/object_admission` now globally reverses a B-rep **only** when the exact
spatial query returns `Inward`. `Outward`, `NotSolid`, and `Unknown` retain their
supplied sense; non-B-rep geometry, including mesh winding, is untouched. There
is no signed-volume fallback and connected shells are never flipped separately.
In-place reversal changes face flags without cloning/reallocating surfaces,
trims, edges, or vertices.
The curved support query also cheaply rejects provably unattainable hull-bound
contacts before constructing rational jets; this avoids an admission-time
regression on periodic point-grid surfaces without weakening its exact witness.

Add, explicit replacement, and explicit replacement-geometry copies share this
policy. All editability and group checks precede geometry work, and all fallible
staging precedes document/history mutation. Replacement normalization happens
before the equality check: `ChangesOnly` retains geometry snapshot identity,
redo history, and selection for normalized no-ops; `EveryReplacement` still
records explicit assignments. Undo/Redo restores the recorded immutable
snapshots without reclassifying geometry. Transform/morph and their copy paths
retain their existing material-side behavior, separate from explicit admission.

The [updated 64-case replay](../tools/rhino_oracle/observations/document_brep_after_report.json)
has **56 complete matches and eight coincident-shell differences**, with no
native failures or numeric errors. All 40 normalization-only gaps are resolved.

A fresh [16-case import request](../tools/rhino_oracle/fixtures/document_brep_import.json)
and [full Rhino capture](../tools/rhino_oracle/observations/document_brep_import.json)
separately test `RhinoDoc.Import` in disposable headless documents, in an owned
private-Xvfb session. The native adapter executes actual `Import3dm`. The
low-level `File3dm` source remains unchanged, but document import globally
normalizes inward solids just like Add. Source and destination units are
millimeters. This is file-import evidence, not an Open-command or unit-conversion
audit; each adapter disposes its own document/artifacts.

Import improves from [nine matches/seven differences](../tools/rhino_oracle/observations/document_brep_import_before_report.json)
to [14 matches/two differences](../tools/rhino_oracle/observations/document_brep_import_after_report.json).
The remaining coincident-shell getter/sense differences are preserved explicitly;
all numeric fields match exactly. Low-level 3dm readers and writers still retain
raw face orientation; it is admission to a document that now normalizes known
inward solids. No import-specific bypass or file-data rewrite was added.

Native regressions also cover rational spheres, quadratic trims deliberately
outside classifier support, underflow/overflow volume scales, identity and
history sharing, atomic permission failures, and explicit-copy group ownership.
The first two normalization regressions failed before the implementation change.
These native cases are not additional Rhino observations. The Rhino matrices do
not cover curved B-reps, meshes, extreme scales, transforms, or Undo/Redo.
[Implementation/import provenance](document-normalization-provenance.json)
records the new capture, retained comparisons, and implementation source hashes.

To recapture both engines, use an owned private Xvfb session:

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/document_brep.json \
  --absolute-epsilon 0 --relative-epsilon 1e-12 --timeout 1800
python3 -m unittest tools.rhino_oracle.test_document_brep
```

Comparison mode assigns unique temporary artifact paths, never overwrites a
caller-owned file, and keeps all sources alive through both engines. A direct
Rhino run without native-prepared shared artifacts is deliberately rejected.

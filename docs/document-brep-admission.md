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
- Forty regular solid workflows differ because native insertion/replacement
  currently preserves inward inputs, while Rhino globally reverses them.
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
assert the 40 normalization-only gaps and eight coincident-query gaps before
checking every remaining field; these gaps are not counted as matches.

## Scope and implementation boundary

This audit does not change document admission policy. Imports currently share
the native insertion API; file reading, document import, explicit replacement,
transform/copy, and history restoration must not be conflated. A `File3dm` read
preserving source geometry does not establish Rhino document Open/Import policy.
The native conservative spatial classifier can also return `Unknown`, including
opposed coincident shells. Neither case justifies a total-volume fallback or
independently flipping each connected shell, which would destroy cavity sense.
The matrix does not cover curved B-reps, meshes, or extreme numeric scales.

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

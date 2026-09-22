# Orientation and document insertion audit

`orientation_audit` is a Rhino-only diagnostic, not an operation accepted by the
native oracle. Its source-only generator constructs geometry through public
RhinoCommon APIs in an owned private-Xvfb session. It separately records source
geometry, document insertion, reversal followed by replacement, and actual `Flip`
command results. It never decompiles or inspects proprietary code.

The retained [request](../tools/rhino_oracle/fixtures/orientation_audit.json) and
[observations](../tools/rhino_oracle/observations/orientation_audit.json) contain 36
cases on Rhino 8.32.26160.13001: 20 insertion/replacement cases and 16 actual command
cases. Both generic insertion and the `AddBrep` overload with kinky-face splitting
disabled are tested. Caller-owned source geometry is checked for mutation; object
identity and counts are checked after document edits. Names, group membership,
selection, complete geometric definitions, and named command events are retained.

Observed single-shell behavior:

- Closed boxes and spheres are oriented outward on insertion and replacement.
- Open B-reps preserve their input sense on insertion and accept reversed replacements.
- Meshes preserve input winding, including inward closed meshes.
- `Flip` skips closed B-rep solids and points but flips open B-reps, curves, and meshes.
- Preselected objects stay selected. Postselected flipped objects are deselected;
  skipped objects stay selected, even within a mixed group.

For open B-reps, all geometry except the topology's face-sense flags is identical
before and after `Flip`. Surface UVs, spatial edges, trim curves, and tolerances do
not change. This motivates the native command's independent oriented-face model.

The mesh `SolidOrientation()` diagnostic remains unchanged after raw face winding
has reversed in these captures. It must not override the retained mesh faces as
evidence. This is an observed getter inconsistency, not proof of its internal cause.

The raw response was compacted as JSON, retaining all values including the exact
IEEE-754 bit patterns of every number. Tests verify definitions and selections;
no result is rewritten to agree with a desired native outcome. This is not an
identical-source native geometry comparison or performance benchmark.

Reproduce the primary capture with:

```sh
tools/rhino_oracle/run_headless.sh rhino \
  tools/rhino_oracle/fixtures/orientation_audit.json --timeout 1800
python3 -m unittest tools.rhino_oracle.test_orientation
```

## Compound shells and inconsistent face orientation

The follow-up retains [20 compound requests](../tools/rhino_oracle/fixtures/orientation_compounds.json)
and their [complete observations](../tools/rhino_oracle/observations/orientation_compounds.json).
It varies disconnected and nested boxes, shell senses, relative size, and face
table order. The scalar volume is checked against the independent exact box
integral `(2r)^3`, summed with each source shell's sign.

| Input | Reported solid orientation | Exact signed volume | Insertion |
| --- | --- | ---: | --- |
| Disjoint small outward box, larger inward box | Outward | −56 | Unchanged |
| Reversed nested cavity | Inward | −504 | All face senses flipped |
| Coincident outward then inward boxes | Inward | 0 | All face senses flipped |
| Coincident inward then outward boxes | Outward | 0 | Unchanged |

The table uses analytic volumes, not rounded or rewritten observations. For
example, the first raw volume is `-56.000000000000014`. A global negative-volume
test would incorrectly reverse that source. The zero-volume coincident cases
also show ordering-sensitive classification; the noncoincident cases retain
their orientation classification when the two source tables are exchanged.
In all 20 cases, document replacement of a reversed duplicate restores the
inserted sense, and `Flip` leaves the closed solid selected and unchanged.

A separate [16-case face-sense request](../tools/rhino_oracle/fixtures/orientation_faces.json)
and [clean capture](../tools/rhino_oracle/observations/orientation_faces.json) toggle
one or three faces of a box, with global reversal, both insertion overloads,
and pre/postselection. Every edge still has two uses, but oriented incidence is
inconsistent. Rhino reports `IsSolid=False`, preserves the input on insertion,
and accepts both a reversed replacement and an actual `Flip`. It does not repair
the inconsistent sense. This corrected the native command's guard from
`is_closed()` to `is_solid()`, with a regression test that failed before the fix.
The [public orientation enum](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/T_Rhino_Geometry_BrepSolidOrientation.htm)
distinguishes non-solids from inward, outward, and unknown solid orientations.

The [first face capture](../tools/rhino_oracle/observations/orientation_faces_startup_recovery.json)
is retained separately. Its first history contains queued startup-recovery
commands after `Flip`; an apparently idle window and missing progress file did
not establish that startup had failed. The batch was recaptured without manual
input. Tests reject the contaminated history, compare the two sets of geometry,
and use only the clean batch as primary evidence. The earlier 300-second compound
attempt timed out; the completed 20-case run had an 1800-second limit. These
harness durations are not kernel benchmarks.

The [follow-up provenance](orientation-followup-provenance.json) records hashes
before and after lossless JSON compaction. Reproduce with:

```sh
tools/rhino_oracle/run_headless.sh rhino \
  tools/rhino_oracle/fixtures/orientation_compounds.json --timeout 2400
tools/rhino_oracle/run_headless.sh rhino \
  tools/rhino_oracle/fixtures/orientation_faces.json --timeout 900
python3 -m unittest tools.rhino_oracle.test_orientation_followup
```

Document-wide normalization remains separate from [`Flip`](commands/flip.md).
The counterexamples rule out a volume-sign shortcut; they do not establish a
general spatial classification algorithm. The kernel's new shared-edge face
component query preserves disconnected shells and supplies deterministic
topology for that work. It is also used by automatic joining, replacing a
duplicated adjacency traversal, but does not classify containment or orientation.

## Spatial-axis follow-up

The [24 spatial requests](../tools/rhino_oracle/fixtures/orientation_spatial.json)
and [observations](../tools/rhino_oracle/observations/orientation_spatial.json)
vary X/Y/Z separation, relative box size, global sense, and source-table order.
Each source contains two disjoint, oppositely oriented boxes of half-size 1 and 2.

- For X separation, the reported orientation follows the lower-X box, regardless
  of size or table order.
- For Y and Z separation, it follows the larger box, regardless of table order.
  That box extends farther in X even when it has the higher Y or Z coordinate.
- Insertion leaves outward-classified sources unchanged and globally reverses
  inward-classified sources; complete definitions otherwise remain identical.

These observations are consistent with selecting the unique minimum-X shell in
these fixtures. They do **not** establish a general algorithm or an equal-extremum
tie policy, and do not cover curved, trimmed, or intersecting shells. No native
classification heuristic or document normalization was introduced from this batch.

The probe now records definitions directly, without computing and then deleting
edge, surface, and lifted-trim samples. Sampling remains enabled by default for
interchange callers. The optional `measure_volume=False` skips mass integration;
its retained `volume=null` means unmeasured, not zero. All older request defaults
are unchanged. Four repeated X-axis cases match the earlier compound definitions
and orientation records exactly, apart from the explicitly unmeasured volume.
Tests also prove that the definition-only path makes no sample-evaluation calls.
These changes reduce unnecessary work; this is not a kernel-speed benchmark.

The [spatial provenance](orientation-spatial-provenance.json) records capture
source hashes and lossless-compaction checks. Reproduce with:

```sh
tools/rhino_oracle/run_headless.sh rhino \
  tools/rhino_oracle/fixtures/orientation_spatial.json --timeout 900
python3 -m unittest tools.rhino_oracle.test_orientation_spatial
```

## Shared-source kernel query

The subsequent [49-case comparison](solid-orientation.md) asks both engines about
the same native-exported 3dm B-reps, without document insertion. A conservative
classifier now matches 47 complete records and all 49 geometry records;
coincident opposed shells remain `Unknown` natively. The
[exact planar path](planar-solid-orientation.md) resolves the corner-only
tetrahedron and retains a further 68-case audit, including translation-sensitive
Rhino classifications repeated in a fresh session.
The coincident-shell Rhino classifications differ from the independently
constructed boxes above. Their representations differ, so the original table
must not be read as a representation-independent source-order tie rule.
Both captures remain intact. The new query is not document normalization.

The subsequent [shared-source document admission audit](document-brep-admission.md)
compares actual insertion and replacement on identical native-exported B-reps,
including separate original-source and reversed-source witnesses. It does not
infer import, transform, or undo policy from those public object-table calls.

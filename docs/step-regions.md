# Curved STEP material regions

Build the application with `--features native-smlib` to preserve closed curved
cavities in `ExportStep Native=Yes`. The optional I/O feature of the same name
also enables this path for the library writer. Default builds retain the exact
convex planar certificate and continue to emit unsupported curved shells as
separate surface models.

The SMLib importer now returns the parent forest produced by its existing
component intersection and containment checks. Each outward shell starts one
material region, with its immediate inward cavities; nested outward islands
start separate regions. Rust checks the returned indices, alternating sense,
acyclic forest and complete component partition before using the plan. The
exporter uses original Rust geometry throughout. The temporary native solid is
released after classification; no fitted or rebuilt native geometry enters the
file or document.

Classification retains the existing limits of 128 components and 65,536
component face pairs. Touching, intersecting, inconsistently oriented or
unclassified shell arrangements fail explicitly before file replacement.
Individual shell self-intersection certification, larger arrangements and
model-scale extremes remain unqualified. Native support is currently Linux-only.

## Canonical void orientation

The STEP schema requires `orientation = FALSE` on void shells; see
[ISO 10303-42's void definition](https://steptools.com/stds/smrl/data/resource_docs/geometric_and_topological_representation/sys/6_schema.htm)
and [the advanced B-rep representation rule](https://ap238.org/SMRL_v8_final/data/resources/aic_advanced_brep/aic_advanced_brep.htm).
The local Monstertruck writer previously emitted `TRUE`. It now writes `FALSE`
and reverses the underlying face sense and face-bound sense together, preserving
the caller's effective material orientation. This correction also applies to
the default convex planar solid exporter. Surface geometry and exact p-curves
are unchanged.

## Reproduce the artifacts

```sh
GIT_LFS_SKIP_SMUDGE=1 git submodule update --init third_party/usd-brep
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo run -p viboceros-io \
  --features native-smlib --example step_regions --release -- \
  /tmp/viboceros-step-regions-owned
WINEDLLOVERRIDES='sspicli,secur32,schannel=b' \
  tools/rhino_oracle/run_headless.sh rhino \
  /tmp/viboceros-step-regions-owned/request.json \
  --scheme VibocerosOracleStepRegionsOwned --timeout 300 \
  --output /tmp/viboceros-step-regions-native.json
```

The seven fixtures cover a box/sphere cavity, concentric curved shells, an
island in a void, a cavity in a nonconvex torus tube, a separate sphere in the
torus hole, two cavities in a box, and a cone/sphere cavity. The example places
inner shells first to exercise ordering independent of the original face table.
Library tests also reverse component order, check units and source preservation,
and reject tangent, crossing and inconsistent shell arrangements. Command tests
check one-object cavity reimport, undo/redo and atomic export failure.

## Independent reader and Rhino diagnostics

`tools/usd_brep/inspect_step_regions.cpp` uses Open CASCADE's public STEP reader,
`BRepCheck_Analyzer` and adaptive volume integration. It writes one record per
material solid. The qualification uses OCCT 7.6.3 development/runtime packages
extracted into a temporary directory; no OCCT dependency is added to the
application. To build against an installed OCCT development package:

```sh
c++ -std=c++17 -O2 -I/usr/include/opencascade \
  tools/usd_brep/inspect_step_regions.cpp -o /tmp/inspect_step_regions \
  -lTKSTEP -lTKSTEPBase -lTKSTEPAttr -lTKXSBase -lTKShHealing -lTKTopAlgo \
  -lTKBRep -lTKGeomBase -lTKG3d -lTKG2d -lTKMath -lTKernel
/tmp/inspect_step_regions /tmp/viboceros-step-regions-occt.json \
  /tmp/viboceros-step-regions-owned/*.step
python3 tools/usd_brep/compare_step_regions.py \
  /tmp/viboceros-step-regions-native.json \
  /tmp/viboceros-step-regions-owned/properties.json \
  /tmp/viboceros-step-regions-comparison.json \
  --occt /tmp/viboceros-step-regions-occt.json
```

OCCT validates the expected material solids and their volumes. Rhino's public
STEP reader imports cavity regions as blocks containing separate outward bodies,
losing their material subtraction. The probe expands those instance definitions
with their accumulated placements, retains top-level types separately and checks
actual boundary geometry. It neither joins the bodies nor subtracts their volumes
to present an artificial native success. Use 3DM for curved cavity transfer into
this Rhino version. The retained comparison distinguishes independent material
qualification from Rhino's shell-geometry diagnostics.

The final capture qualifies seven OCCT cases containing nine material solids.
Rhino contributes 4,950 edge and surface-lifted boundary samples; the comparison
records six failed native material transfers separately. These fixtures do not
establish general geometry equivalence or eliminate the remaining model limits.

[Exact inputs and diagnostic logs](step-regions-diagnostics/),
[independent capture](step-regions-occt-reference.json),
[Rhino capture](step-regions-native-reference.json),
[comparison](step-regions-comparison.json) and
[source/check provenance](step-regions-provenance.json) retain the checkpoint.

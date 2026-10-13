# Modelling issue fixes

Follow-up to the supplied 12 October 2026 ASTRA/LMP2 modelling report.
The user's source models remain external fixtures; repository tests retain small
independent constructions and the reduced fender inputs.

| Issue | Change | Verification |
| --- | --- | --- |
| VIB-01 | Chunk all viewport draw buffers within the device limit; normalize stale imported trim flags | Original 948-object master, 4,801 faces, zero skipped objects; three Wireframe/Shaded/Ghosted cycles with 316,020,000 bytes of triangle vertices; enforced 1 MiB GPU-limit regression |
| VIB-02 | Split composite B-rep edges and every incident trim at export, retaining support surfaces | Certified rational ellipse → closed trimmed panel → 3DM; independent OpenNURBS validity/closure audit, maximum deviation `4.78331e-16` |
| VIB-03 | Require the native projection distance to satisfy the existing tolerance before trimming intersection curves; preserve legal native endpoint-zone bounds locally | Ordinary and formerly failing boundary-crossing lamp cutters pass native endpoint checks → strict portable B-rep → 3DM in both postprocessing modes |
| VIB-04 | Preserve object UUIDs, including block members; fresh IDs for conflicts and duplicates | Actual ASTRA V3: 84/84 IDs retained; actual LMP2 V3: 467/467 IDs retained; geometry, names, layers and groups unchanged; repeated import and undo/redo regressions |
| VIB-05 | Add `BlendSrf` with independent G0/G1/G2 targets, bulges, orientation, preview, rectangular trimming and atomic joining | Supplied ASTRA collar transition agrees within `1e-12` on a 41×41 grid, including second derivatives; command, GUI, undo and 3DM regressions |
| VIB-06 | Add optional per-corner normals, face provenance, local UV/origin charts and constrained trimmed-knot creases | Analytic derivatives, face reversal, singular limits, loop-order independent creases, closed-solid seam audits, conforming samples and large parameter domains |

VIB-03's retained larger lamp cutter produced native edge/vertex gaps up to
`4.7084e-3` with radii near `1e-9`, before copying or portable extraction. The
kernel accepted off-curve projections as permission to trim intersection curves.
The bounded source fix uses the existing tolerance, leaving the upstream
submodule immutable. Both Boolean postprocessing modes now pass native endpoint
checks and strict portable/3DM validation. The original failed experiment's
script was absent from the supplied archives; the regression reconstructs the
surviving source fender and a reduced cutter. See the
[native fixture and diagnosis](native-lamp-well.md).

`BlendSrf` currently accepts complete rectangular/isoparametric single-face
boundaries; arbitrary UV trim chains and draggable handles remain pending.
See the [command reference](commands/blend-srf.md) for its options and limits.

The [display diagnostics](display-mesh-diagnostics.md) explain the original GPU
workload: 2,633,500 triangles need 301.38 MiB, which the old allocation path rounded
to 512 MiB. The six cabinets' dense fillet grids contribute 1,439,340 triangles;
the two forks' many-span trim curves dominate meshing time. Display sampling
has not been reduced globally.

The [normal-channel API](tessellation-normals.md) keeps local UV samples and their
origins separately; adding them in f64 can lose parameter stations. Singular
samples do not discard regular normals elsewhere in the object. Optional normal
meshes preserve trimmed knot creases using audited constraints. Missing shared
crease anchors return an explicit unsupported error; split the face at that knot
before requesting the channels.

Validation includes 2,175 geometry tests, 1,277 command tests, 1,115 app tests,
224 IO tests, the real-model GPU/identity checks above, native-feature build and
BlendSrf checks, and native Boolean/portable-transfer regressions. Existing
ignored tests remain opt-in; the GPU regressions were run explicitly.

Re-run the external-model checks without committing a model:

```sh
VIBOCEROS_ASSEMBLY_3DM=/path/to/ASTRA-07-Native-Master.3dm \
  cargo test --release -p viboceros large_native_assembly_repeated_display_switches \
  -- --ignored --nocapture
VIBOCEROS_ASSEMBLY_3DM=/path/to/model.3dm \
  cargo test -p viboceros-command external_assembly_open_save_as_preserves_ids_and_geometry \
  -- --ignored --nocapture
```

The first check needs a graphics adapter. Generated roundtrip files are written
to temporary directories, leaving the source archive unchanged.

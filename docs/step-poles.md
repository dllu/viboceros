# STEP singular pole trims

Editable STEP export now preserves certified collapsed boundaries on NURBS
surfaces, including sphere poles, cone apices and polar disk centers. The writer
adds a constant degree-one spatial spline with one shared endpoint vertex and
retains the exact face-local UV p-curve. It continuously certifies that the whole
UV curve maps to that vertex within the allowed model/vertex tolerance before
emitting the file. Normal closed circles and periodic seam edges retain their
ordinary edge identity.

The reader recognizes a collapsed support only when both endpoint indices and
all Euclidean control points agree, with sign-coherent rational weights. Each
incident UV curve must also pass continuous surface-image certification. It
then restores a singular trim without a spatial edge. This prevents an ordinary
closed curve or a constant support with a noncollapsed UV image from silently
losing geometry. Existing staged export replacement remains atomic.

The STEP representation follows the general spatial spline plus surface
p-curve approach described by the public
[Open CASCADE STEP edge translator](https://github.com/Open-Cascade-SAS/OCCT/blob/master/src/DataExchange/TKDESTEP/TopoDSToStep/TopoDSToStep_MakeStepEdge.cxx).
The implementation was written independently; no translator code was copied.
Elementary spherical STEP faces with angular trims touching a pole remain a
separate unsupported import case. This change supports the explicit NURBS
surface/p-curve representation used by our writer.

## Reproduce

```sh
cargo run -p viboceros-io --example step_poles --release -- \
  /tmp/viboceros-step-poles-owned
WINEDLLOVERRIDES='sspicli,secur32,schannel=b' \
  tools/rhino_oracle/run_headless.sh rhino \
  /tmp/viboceros-step-poles-owned/request.json \
  --scheme VibocerosOracleStepPolesOwned --timeout 300 \
  --output /tmp/viboceros-step-poles-native.json
python3 tools/usd_brep/compare_step_poles.py \
  /tmp/viboceros-step-poles-native.json \
  /tmp/viboceros-step-poles-owned/properties.json \
  /tmp/viboceros-step-poles-comparison.json
```

The example uses the retained native SMLib pocket/cavity 3DM fixtures along with
Rust sphere/cone primitives; running it requires OpenNURBS but no native SMLib
build. The owned, empty Rhino document is checked before import, and imported
objects are deleted afterward. The probe calls the public
[`FileStp.Read` API](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_FileIO_FileStp_Read.htm)
with explicit join/no-face-limit options. The initial `RhinoDoc.Import` attempt
waited at its interactive options prompt and timed out; its log and screenshot
are retained separately.

## Qualified results and limits

Rhino 8.32.26160.13001 imported the sphere, cone and hemispherical pocket as one
valid outward solid each. Their edge and surface-lifted trim samples agree with
the expected analytic boundaries within 1e-8; the largest observed deviation
across all four fixtures was 3.6e-15 over 3,300 samples. Native volume queries
agree with the three source solids within 1.3e-8 relative. These samples and
properties qualify these fixtures, not arbitrary pole geometry or complete
interactive Rhino equivalence.

The spherical cavity remains a limitation: the existing STEP writer emits its
two disconnected shells as separate surface models, and Rhino imports two
outward solids. Its void relationship and source material volume are lost.
The boundary samples agree, but this fixture is explicitly **not** qualified
as a solid transfer. Use 3DM to retain curved cavity semantics. General curved
`BREP_WITH_VOIDS` certification and grouping remain further work.

[Exact STEP inputs and logs](step-poles-diagnostics/),
[native capture](step-poles-native-reference.json),
[comparison](step-poles-comparison.json) and
[source/check provenance](step-poles-provenance.json) retain the evidence.
Local regressions cover sphere/cone poles, reversed orientation, source units,
ordinary closed-circle preservation and malformed UV correspondence. The native
SMLib transfer suite also checks sphere and pocket STEP round trips.

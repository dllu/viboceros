# Native general surface intersections

Linux `native-smlib` builds extend Intersect, IntersectTwoSets and the surface
cutter query used by Split with usd-brep's `SmSurface::GlobalSurfaceIntersect`.
The established exact analytic dispatcher runs first, preserving its specific
coincidence and singularity restrictions. Its unimplemented general nonplanar
pair branch can use the native producer, including polynomial patches such
as `z=x²+y³` against `z=1−xy`. Surface/B-rep and B-rep/B-rep queries admit this
producer for face pairs covering both complete natural domains. Existing
analytic trimmed-face handling retains its shared path.

Each native query imports temporary source surfaces with the existing B-rep
importer. It returns independent snapshots of the spatial curve and both UV
curves, classified as touching points or curve contacts. Snapshot ownership
transfers once; untaken snapshots are destroyed with the event owner. Source
buffers, kernel pointers and UV witnesses do not enter document history. The
bridge serializes calls and destruction, validates classifications and array
counts, and clears outputs on invalid inputs. A touching point may have null
UV entries in the upstream result; the bridge handles that explicitly.

Preparation reuses the curve/surface adapter's common weight normalization,
translation, uniform spatial scale, unit UV charts, endpoint clamping and rank
checks. Returned XYZ controls and both UV control nets map back to the original
inputs. Source degrees above 16 and unsupported preparation retain the portable
producer. Native solving and certificate failures on eligible inputs propagate.

Every curve requires `parameter_curve_deviation_bound` on **both original
surfaces**, using the existing effective absolute/relative model-space limit.
Exact rational extraction and outward interval or exact Bernstein arithmetic
prove the complete matched-parameter correspondence and that UV stays inside
each natural domain. An inconclusive certificate rejects the proposal. The
existing certificate has bounded work, surface/UV degrees up to 16, spatial
degree up to 64, and bounded composed degree. Point contacts are independently
checked on both original sources. Certified curve payloads remain verbatim;
direct proposals are not joined by moving their endpoints. Measured analytic
curve definitions still use their established assembly policy.

General partial face trims are deliberately unavailable in this increment.
The tested polynomial pair produced an approximate spatial curve whose
intersections with exact trim edges were missed by both existing spatial
contact producers. A midpoint-only trim classification then retained an
outside segment. Such pairs now return the established unsupported error
before editing the document. Correct extension requires clipping the coupled
UV witnesses against original trim loops, preserving both spatial/UV parameter
correspondence and boundary contacts.

[Retained qualification](smlib-surface-surface-intersections-comparison.json)
contains 42 actual C ABI cases in normal and complete AddressSanitizer builds:
transverse planes, empty pairs, coincident boundaries, a coplanar corner touch,
saddle crossings, a hyperbolic section and the general polynomial pair. Each
uses independent source charts in three planes. Independent image formulas,
unchanged source buffers, invalid inputs, transfer-once ownership and snapshot
evaluation after event destruction are checked. These sampled diagnostics
supplement the Rust continuous acceptance checks. Leak accounting for the
process-global context alone is disabled.

Rust regressions also cover independent charts, rotated/translated images,
common positive and negative extreme weight gauges, false XYZ/UV proposals,
portable fallback and producer errors. Commands verify full-domain surface and
face outputs, operand preservation, selection, Undo/Redo and atomic rejection
of general partial trims. Raw kernel output can depend on its parameter charts
and approximation settings. The saddle probes return two or four curves;
the normalized Rust query includes oppositely directed approximate duplicates.
Continuous acceptance preserves their payloads and does not infer topology.
Qualification passes 268 native adapter tests, 91 native intersection command
tests, 24 cutting Split tests, three acceptance-hook tests, and native/portable
lint with warnings denied. Revised producer tests are also rerun separately.

The comparison replays all 121 saved intersection/cutting command records at
absolute `1e-8`, relative `1e-12`, retaining complete native/portable responses
and SDK comparisons. These saved observations check existing compatibility;
the polynomial regression is an independent analytic test, not a Rhino capture.
Fresh instrumentation remains blocked by the existing Wine licensing startup
failure. Continuous curve acceptance does not prove complete event discovery,
topology or injectivity. Arbitrary conditioning and speed relative to Rhino
remain unqualified.

```sh
cargo test -p viboceros-smlib --features native --test surface_intersections
cargo test -p viboceros-geometry --lib surface_surface_backend
cargo test -p viboceros-command --features native-smlib --lib curve_intersections::tests
cmake -S tools/usd_brep -B /tmp/surface-contacts -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DVIBOCEROS_CURVE_INTERSECTION_DISTANCE_PATCH=ON \
  -DVIBOCEROS_CURVE_SELF_INTERSECTION_PAIRS_PATCH=ON
cmake --build /tmp/surface-contacts --target viboceros_usd_brep_surface_surface_adapter_probe
/tmp/surface-contacts/viboceros_usd_brep_surface_surface_adapter_probe
```

Use `RelWithDebInfo`, compiler flags `-fsanitize=address -fno-omit-frame-pointer`
and linker flags `-fsanitize=address` for ASan. See
`python3 -m tools.usd_brep.qualify_surface_surface_intersections --help` for the
retained comparison inputs. The separate `viboceros_usd_brep_surface_surface_probe`
target inspects upstream XYZ/UV arrays before adapter export.

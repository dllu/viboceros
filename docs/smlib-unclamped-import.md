# Unclamped native B-rep import

The native B-rep adapter accepts unclamped NURBS in 3D edges, UV trims and
surface faces. Previously, it rejected them before calling the kernel, so a
valid unclamped sheet could not participate in native Join or JoinCopy.

The importer passes the full source data to usd-brep's existing
`SmBSplineCurve::CreateCanonical` and `SmBSplineSurface::CreateCanonical`.
Their IGES import routines clamp the active interval. No fitting or second
clamping implementation is added. After construction, the adapter checks the
degree and retains the active endpoints and every interior knot through the
existing basis-preservation helper. Full-order interior discontinuities remain
rejected. Positive finite weights and the existing dimension/resource limits
remain required.

An unclamped source's exterior knots and inactive controls can disappear in
the native modeling copy. Its original Rust object stays unchanged. Native
orientation can still transpose a face chart, and stitching retains its
documented tolerance and topology policies. This change does not make the
portable backend's raw Rhino comparison expectations apply to native builds.

The bridge regression checks 75 rational surface cases spanning clamped and
fully or partially unclamped axes, different control counts and parameter scales
`2^-40`, `1` and `2^40`. Each exported tensor definition is compared with an
independent Rust knot-insertion result, including knots, controls and weight
ratios, and a grid on the original active domain. The largest sampled point
error is below `1e-15`. Additional cases check unclamped edge/UV combinations,
a periodic seam and closed-solid import followed by a Boolean cut.

Sixteen native Join/JoinCopy workflows check unclamped U/V charts, reversed
faces, preselection and command-first picking. Full coefficients agree with
the independently authored active sheets, allowing the native chart transpose.
Source geometry, metadata, groups, selection and two Undo/Redo cycles are
checked. These cases do not establish arbitrary trimmed or periodic topology,
all numerical ranges, raw Rhino parameterization parity or performance parity.

All 48 saved curved-Join recipes execute, including eight unclamped recipes.
Four match the complete raw Rhino record; 44 retain tolerance, representation
or other field differences. An independent 80-digit tensor evaluation compares
4,096 active-surface samples across the eight unclamped recipes within `1e-10`,
requiring a unique face pairing and allowing only the native U/V transpose.
The same witness rejects a deliberately moved reference control point.
This sampled witness does not replace the strict comparison or prove whole
B-rep boundary equivalence.

The separate 64-case interior-isocurve audit executes 32 cases, including 16
unclamped cases. Its other 32 cases, with UV origins near `1e12`, still fail
with an invalid active-domain error. Those failures are retained as errors;
large translated charts remain further work.

```sh
cargo test -p viboceros-smlib --features native --test unclamped_brep
cargo test -p viboceros-command --features native-smlib --test native_unclamped_join
python3 -m tools.rhino_oracle audit tools/rhino_oracle/fixtures/join_curved_certificates.json --native-smlib
```

The audit command retains execution outcomes. Strict comparison with the saved
Rhino records also checks native tolerance and representation differences; an
executed operation alone is not a matching result. The
[validation record](smlib-unclamped-import-validation.json) retains the obtained
test and audit evidence.

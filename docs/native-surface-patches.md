# Exact native surface patches

Native rectangular extraction retains the requested in-domain surface region,
including boundaries close to existing knots. Generated-source repairs apply to
`N_SrfExtractPatch` and `SmBSplineSurface::CopySubPatch`; the pinned upstream
checkout is unchanged.

The original extractor snapped each boundary using an absolute `1e-8` parameter
tolerance, then mapped the snapped surface back to the original parameter box.
A bilinear source `S(u,v)=(u,v,0)` requested at U start `1e-9` therefore reported
that start while its boundary lay at X `0`. Thin valid intervals could collapse.
Its active-end shortcut also assumed clamped padding, retaining the wrong rows
at unclamped ends and at full-order groups with inactive outer controls.

The repair locates exact knot groups and uses their actual multiplicities for
the existing two-axis homogeneous restriction. It removes snapping, remapping
and the artificial clamped-end indices. Full-order interval starts retain the
right-hand control block and interval ends retain the left-hand block. No new
basis evaluator, refinement solver, geometry fit or tolerance change is added.
The previous finite extrapolation route outside the active domain is retained
without a new accuracy claim.

`CopySubPatch` still intersects the requested rectangle with the natural domain.
It now copies the extracted tensor through the supported exact setter on a
fresh owning surface. This avoids the raw-data constructor's knot repair while
retaining a deep copy of the temporary stack result. A local cleanup guard owns
the new surface until construction succeeds; its context and derived state are
fresh.

The direct probe independently authors restriction coefficients using linear
and quadratic Bernstein polar forms, uniform quadratic span definitions and
isolated control blocks. Its 256 cases vary axes, polynomial/rational controls,
common weight signs, binary chart scales `2^-40`, `1`, `2^40`, and origins `1e12`
and `-2e12`. Cases include thin intervals, a station immediately after a knot,
whole unclamped domains, inactive controls and full-order boundary/interior
groups. Every output degree, count and full knot vector is exact. Every
homogeneous coordinate is checked at `2e-13 * max(1, abs(expected))`. Out-of-place
inputs remain unchanged. Owning object outputs are reread after destroying the
source object; in-place inputs are consumed according to the native contract.

The opt-in CMake target `viboceros_smlib_surface_patch_baseline` overrides only
the two changed native objects with their preceding generated versions. It
retains the earlier isocurve, copy and U/V repairs. The
[validation record](native-surface-patches-validation.json) retains control,
candidate and instrumented output, completed jobs, SDK comparisons and hashes.

Fresh Rhino capture covers eight continuous rational fixtures. Six complete
definitions match in each native path. The two whole-domain fixtures retain an
unclamped basis in Rhino; rectangular native extraction clamps both directions,
so their raw definitions remain different. Both retained definitions were also
evaluated by Rhino on matching 9-by-9 parameter grids. Points, first derivatives
and normals agree within `5.6e-16` at all 162 paired stations. These are sampled
geometry checks, separate from the unchanged strict definition reports.

All 256 cases pass through all three paths under AddressSanitizer. Leak detection
is disabled for the process-global API context; this checks invalid memory
access and allocation lifetime. The original-object control passes 128 cases in
each NLib path and 80 in the owning object path. A failed combined lifetime
check on that control can reflect a missing or invalid output; it does not
establish a pre-existing dangling allocation.

Release validation passes 412 bridge tests and 1,600 command tests, with zero
failures and four existing ignored tests. Workspace lint with warnings denied
and formatting pass. All 112 saved raw join outcomes remain unchanged; their
existing strict Rhino mismatch reports are retained separately.

Rectangular extraction clamps both directions. SrfSeam's checked directional
adapter remains necessary to preserve every knot and control layout of the
unselected direction. Its chart and weight conditioning remain unchanged.
General arbitrary weight/chart ratios, out-of-domain extrapolation, native ray
queries and full Rhino command parity remain unqualified. No performance or
leak-free claim is made.

```sh
cargo test -p viboceros-smlib --features native \
  --test surface_patch --test surface_seams --test isocurve_pairs
```

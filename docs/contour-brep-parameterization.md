# B-rep Contour parameterization

All 24 recorded native recipes pass strict comparison, including domains,
ordered samples, direction/seams, properties and group indices at 1e-9 absolute
plus 1e-12 relative epsilon. See [comparison](contour-brep-comparison.json) and
[provenance](contour-brep-provenance.json).

B-rep Contour uses the validated trimmed intersection geometry and restores its
face-local carrier domains and traversal. Untrimmed faces use the surface
parameter policy directly. Face reversal reverses both curve direction and the
native interval. Multiple clipped straight pieces, or a joined line overlapping
an inner boundary, use chord-length intervals starting at zero. Clipped pieces
are ordered in decreasing carrier parameter.

Coincident planar faces emit their actual outer and inner trim boundaries.
Linear loops use chord-length parameters. Inner/outer winding and reversed-face
orientation remain explicit. Multi-face closed polyline cuts use exact dyadic
signed area to determine winding, a sign-independent seam plane, and the last
participating face's candidate seam. Seam selection uses a canonical normal sign while the requested cutting normal
controls traversal. Front/back-facing candidate endpoints and the midpoint of
a back-facing coincident loop retain the recorded native seam.
The native box source used by the oracle
mirrors the SDK face order and physical UV domains, so those are the same
input to the two implementations.

The new native corpus includes full-domain and trimmed faces, a rectangular
hole, an asymmetric hole, input attributes, reversed faces, singleton grouping,
axis/diagonal/reversed cutting directions, offset origins, asymmetric boxes and
coplanar cuts. Public SDK per-face cuts and type/control/domain metadata are
retained separately from command records.

The experiments also expose a kernel defect in trim scan classification. A
closed trim seam must count as one cyclic point. Treating its two parameter
endpoints independently left an unmatched crossing when the seam bordered a
coincident scan-line span. That extended an outer trim past its boundary and
truncated the retained region beside a hole. For a seam bordering an exactly coincident scan-line span, classification now
uses the signs on both sides of the closure and counts the duplicated end only
once. Other closed curves retain their existing endpoint convention. Two direct
face-membership regressions cover both failures.

The oracle samples exact interval endpoints. Reconstructing the last sample as
`start + (end-start)` can round outside an asymmetric interval by one ulp and
produce a false failure; the sampler now uses the stored end value.

Broader curved/multi-face families, native seams for general topology, extreme
UV values, remembered options, section styles/hatches and performance parity
remain open. The native box fixture and the Box command now share a command-compatible
constructor with SDK topology and UV intervals. The generic normalized box
constructor retains its separate convention; see [Box topology](box-native-topology.md).

Grouping is applied only when a plane has at least two outputs. The native
singleton tests cover points, surface curves and closed box contours; none gets
a group even when the option is enabled.

The complete application regression caught a circular-hole Boolean change when
the cyclic rule was initially applied to every closed curve. Restricting it to
an exactly coincident seam span retains the existing curved endpoint convention;
the circular-hole Boolean is included in the final workspace validation.

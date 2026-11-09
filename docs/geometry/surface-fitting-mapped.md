# Explicit-basis image fitting

[Automatic image fitting](surface-fitting-images.md) · [Mapped image bounds](surface-mapped-images.md)

`try_fit_surface_image_in_basis(source, degree, knots, correspondence, limit)`
interpolates Cartesian source points at the supplied basis's tensor Greville
stations, then certifies the complete source/output images under supplied
piecewise affine U/V mappings. A successful `BoundedSurfaceImageFit` contains
polynomial geometry and a complete symmetric Hausdorff bound. Source domains
remain unchanged; normalized parameter speed can change.

Each correspondence list contains strictly increasing normalized
`[source, candidate]` pairs from `[0,0]` to `[1,1]`. Maps and tolerance are
validated before solving. The existing explicit-basis degree, control, knot
and source-weight limits apply. Zero is strict. `None` means the supplied
correspondence is outside the limit or inconclusive; it does not prove that
the surfaces have different images. Invalid inputs and solver failures return errors.

`try_fit_surface_image_in_basis_with_solver` shares sampling and complete
admission with external solvers. `viboceros_smlib::fit_surface_image_in_basis`
uses two batched usd-brep collocation solves through the Cartesian adapter.
Both retain the same certificate and immutable-input contract.
The result also retains its `SurfaceImageFitCertificate::SeparableUv` maps;
`certificate.verify(source, candidate, limit)` rechecks complete image accuracy.

The local JSON operation `surface_fit_image_in_basis` accepts `surface`,
`degree_u`, `degree_v`, `knots_u`, `knots_v`, `correspondence`, and optional
`absolute_tolerance` (default request tolerance). It returns a surface and
complete bound, or two null fields, plus unchanged `before`/`after` snapshots.

A saved curved rational strip fits a fixed, uniformly quartered cubic basis
in both U/V charts. Its complete image bound passes `0.001` where the existing
parameter-preserving explicit-basis fitter declines. The resulting surfaces
match the saved Rhino output at the separate complete `1e-9` gate, including
3DM round-trips. The [validation record](../surface-mapped-validation.json)
keeps these claims separate from the broader source/output image replay.

This API requires a basis and correspondence; it does not choose them or
qualify Rhino's adaptive fitting policy. Automatic fitters keep their existing
behavior. Trimming, topology and tangent continuity remain outside this certificate.

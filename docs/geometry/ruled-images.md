# Complete ruled-surface image bounds

[Geometry guides](README.md) · [Ordered ruled tensors](ruled-ribbons.md)

`NurbsSurface::ruled_surface_image_deviation_bound(other, limit)` bounds the
symmetric Hausdorff distance of complete untrimmed ruled images. Both tensors
must have degree one and two controls in U, with common-sign weights on each
tensor. Their boundary curves use independently normalized V correspondence.
Invalid limits fail; unsupported or unproved comparisons return `None`.

For fixed V, positive relative boundary weights sweep the entire straight
segment between the two boundary points as U traverses its domain. The same
holds when all weights are negative. Match the same affine segment fraction
on both images. Convexity bounds its displacement by the larger of the two
complete boundary-curve bounds, in both image directions. The curve bounds
use the existing exact rational Bernstein certificates; samples do not admit.

Independent boundary gauges can change the U clock while retaining the whole
image. This method certifies no U correspondence, trimming, normals, topology
or injectivity. Mixed-sign tensors can have poles and decline. Tensors with an
interior U control row decline even if their boundary curves agree.

The `_with_witnesses` variant accepts two optional boundary witness curves.
Each actual boundary is certified against the supplied witness at half the
limit; their outward sum bounds the actual boundary difference. This helps
compare differently refined copies of an original source. Witnesses are not
trusted replacements. Exact rectangular-domain coverage remains necessary
when using complete support images to qualify trimmed B-rep faces.

The local Python operation `surface_ruled_image_bound` accepts `first`, `second`
and optional `absolute_tolerance`. It returns `certified`,
`maximum_deviation_bound` and `u_correspondence_certified:false`.

# Surface curve networks

[Geometry guides](README.md) · [Split command](../commands/split.md)

`Brep::try_split_rectangular_surface_face_with_curve_network` partitions a
rectangular NURBS face using simple closed curves and open curves whose ends
meet the boundary or another cutter. Interior T junctions, transverse crossings,
and overlapping contributors become shared graph edges and vertices. A dangling
end or a cutter coincident with a rectangle side rejects the operation. Each result is a validated one-face B-rep retaining
the original surface and its face orientation.

The implementation in `brep/curve_network` shares the existing rectangular
cutting arrangement. It retains exact spatial NURBS restrictions, obtains UV
curves through the existing surface pullback, and traverses directed edges to
recover cells and holes. Interior junction parameters are computed on the input
curves before endpoint-weight normalization of individual fragments.

R-trees restrict curve comparisons to overlapping control hulls and locate
nearby UV nodes. Mixed-sign rational weights and extreme coordinates take a
conservative fallback; tolerance includes the intersection solver's relative
coordinate scale. Candidate ordering remains stable so existing coincident-edge
and trim-seam policies retain their behavior. An endpoint-pair index restricts
fragment coincidence checks to edges that can share the same nodes.

The network entry point bounds input collection and graph growth using a
64 MiB storage estimate. Both arrangement modes limit candidate curve-pair work
to 16 million comparisons. Tests cover interior junctions, concentric rational
loops crossed by spokes, shared crossing coordinates, reversed orientations,
nonunit rational endpoint weights, dangling ends, conservative hull queries,
and resource limits. Existing command arrangement comparisons remain separate;
the command's boundary-to-boundary admission policy is unchanged.

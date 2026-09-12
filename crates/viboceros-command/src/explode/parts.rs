use super::*;

pub(super) enum ExplodedParts {
    Lines(Vec<LineSegment>),
    Curves(Vec<viboceros_geometry::CurveSegment3>),
    Points(Vec<Point3>),
    Surfaces(Vec<Brep>),
    Meshes(Vec<TriangleMesh>),
}

impl ExplodedParts {
    pub(super) fn report(&self) -> (PartKind, usize) {
        match self {
            Self::Lines(parts) => (PartKind::Polyline, parts.len()),
            Self::Curves(parts) => (PartKind::Polycurve, parts.len()),
            Self::Points(parts) => (PartKind::PointCloud, parts.len()),
            Self::Surfaces(parts) => (PartKind::Polysurface, parts.len()),
            Self::Meshes(parts) => (PartKind::Mesh, parts.len()),
        }
    }

    pub(super) fn into_geometries(self) -> Vec<Geometry> {
        match self {
            Self::Lines(parts) => parts.into_iter().map(Geometry::Line).collect(),
            Self::Curves(parts) => parts
                .into_iter()
                .map(|part| Geometry::from(part.into_curve()))
                .collect(),
            Self::Points(parts) => parts.into_iter().map(Geometry::Point).collect(),
            Self::Surfaces(parts) => parts.into_iter().map(Geometry::Brep).collect(),
            Self::Meshes(parts) => parts.into_iter().map(Geometry::Mesh).collect(),
        }
    }
}

/// Count cheap, exact outputs before materializing geometry. Mesh connectivity
/// still needs decomposition; a single B-rep face is not an Explode input.
pub(super) fn known_output_count(geometry: &Geometry) -> Result<Option<usize>, CommandError> {
    Ok(match geometry {
        Geometry::PolyCurve(curve) => {
            Some(curve.segments().iter().try_fold(0usize, |count, segment| {
                let added = match segment {
                    viboceros_geometry::CurveSegment3::Polyline(polyline) => {
                        polyline.segment_count()
                    }
                    _ => 1,
                };
                count
                    .checked_add(added)
                    .ok_or_else(|| too_many_span_outputs("Explode"))
            })?)
        }
        Geometry::Polyline(polyline) => Some(polyline.segment_count()),
        Geometry::PointCloud(cloud) => Some(cloud.points().len()),
        Geometry::Brep(brep) if brep.faces().len() > 1 => Some(brep.faces().len()),
        _ => None,
    })
}

/// Pure decomposition: no document mutation, selection, attributes, or history.
/// The budget bounds mesh parts; other output counts are preflighted by callers.
pub(super) fn decompose(
    geometry: &Geometry,
    tolerance: Tolerance,
    maximum: usize,
) -> Result<Option<ExplodedParts>, CommandError> {
    Ok(match geometry {
        Geometry::PolyCurve(curve) => {
            let mut parts = Vec::new();
            for (index, segment) in curve.segments().iter().enumerate() {
                let segment = segment.try_reparameterized(curve.segment_domain(index)?)?;
                if let viboceros_geometry::CurveSegment3::Polyline(polyline) = segment {
                    parts.extend(
                        polyline
                            .segments()
                            .map(viboceros_geometry::CurveSegment3::Line),
                    );
                } else {
                    parts.push(segment);
                }
            }
            parts.reverse();
            Some(ExplodedParts::Curves(parts))
        }
        Geometry::Polyline(polyline) => {
            let mut parts = polyline.segments().collect::<Vec<_>>();
            parts.reverse();
            Some(ExplodedParts::Lines(parts))
        }
        Geometry::PointCloud(cloud) => {
            let mut parts = cloud.points().to_vec();
            parts.reverse();
            Some(ExplodedParts::Points(parts))
        }
        Geometry::Brep(brep) if brep.faces().len() > 1 => {
            let mut parts = brep.explode_faces(tolerance)?;
            parts.reverse();
            Some(ExplodedParts::Surfaces(parts))
        }
        Geometry::Mesh(mesh) => {
            // A connected mesh creates no command output, even when the
            // remaining budget is zero. Allow its single analysis component.
            let mut parts =
                mesh.try_explode_pieces(maximum.max(1))
                    .map_err(|error| match error {
                        GeometryError::MeshComponentLimit { .. } => {
                            too_many_span_outputs("Explode")
                        }
                        other => CommandError::from(other),
                    })?;
            if parts.len() <= 1 {
                None
            } else {
                parts.reverse();
                Some(ExplodedParts::Meshes(parts))
            }
        }
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::{CurveSegment3, PolyCurve3};

    fn p(x: f64, y: f64) -> Point3 {
        Point3::try_new(x, y, 0.).unwrap()
    }

    #[test]
    fn mesh_budget_rejects_extra_components_but_allows_connected_noops() {
        let mesh = TriangleMesh::try_new(
            vec![
                p(0., 0.),
                p(1., 0.),
                p(0., 1.),
                p(3., 0.),
                p(4., 0.),
                p(3., 1.),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let connected = Geometry::Mesh(mesh.explode_pieces().remove(0));
        assert!(
            decompose(&connected, Tolerance::DEFAULT, 0)
                .unwrap()
                .is_none()
        );
        let source = Geometry::Mesh(mesh);
        for maximum in [0, 1] {
            assert!(matches!(
                decompose(&source, Tolerance::DEFAULT, maximum),
                Err(CommandError::TooManySpanOutputObjects {
                    command: "Explode",
                    ..
                })
            ));
        }
        let parts = decompose(&source, Tolerance::DEFAULT, 2).unwrap().unwrap();
        assert!(matches!(parts.report(), (PartKind::Mesh, 2)));
        let geometries = parts.into_geometries();
        let Geometry::Mesh(first) = &geometries[0] else {
            panic!("mesh expected")
        };
        assert_eq!(first.vertices()[0], p(3., 0.));
    }

    #[test]
    fn polycurve_flattening_keeps_outer_domains_and_reverses_only_part_order() {
        let line = LineSegment::try_new(p(0., 0.), p(1., 0.), Tolerance::DEFAULT).unwrap();
        let polyline =
            Polyline3::try_new(vec![p(1., 0.), p(2., 1.), p(3., 0.)], Tolerance::DEFAULT).unwrap();
        let curve = PolyCurve3::try_with_segment_domains(
            vec![CurveSegment3::Line(line), CurveSegment3::Polyline(polyline)],
            vec![-6., -2., 4.],
        )
        .unwrap();
        let source = Geometry::PolyCurve(curve.clone());
        assert_eq!(known_output_count(&source).unwrap(), Some(3));
        let parts = decompose(&source, Tolerance::DEFAULT, usize::MAX)
            .unwrap()
            .unwrap();
        assert!(matches!(parts.report(), (PartKind::Polycurve, 3)));
        let geometries = parts.into_geometries();
        let domains = [1. ..=4., -2. ..=1., -6. ..=-2.];
        for (geometry, domain) in geometries.iter().zip(domains) {
            let Geometry::Line(line) = geometry else {
                panic!("flattened line expected")
            };
            assert_eq!(line.domain(), domain);
            for t in [
                *domain.start(),
                domain.start().midpoint(*domain.end()),
                *domain.end(),
            ] {
                assert_eq!(line.evaluate(t).unwrap(), curve.evaluate(t).unwrap());
            }
        }
    }

    #[test]
    fn points_and_polylines_reverse_parts_without_reversing_geometry() {
        let points = vec![p(0., 0.), p(2., 1.), p(4., 0.)];
        let cloud = Geometry::PointCloud(PointCloud3::try_new(points.clone()).unwrap());
        assert_eq!(known_output_count(&cloud).unwrap(), Some(3));
        let parts = decompose(&cloud, Tolerance::DEFAULT, usize::MAX)
            .unwrap()
            .unwrap();
        assert!(matches!(parts.report(), (PartKind::PointCloud, 3)));
        assert_eq!(
            parts.into_geometries(),
            points
                .iter()
                .rev()
                .copied()
                .map(Geometry::Point)
                .collect::<Vec<_>>()
        );
        let polyline = Polyline3::try_new(points, Tolerance::DEFAULT).unwrap();
        assert_eq!(
            known_output_count(&Geometry::Polyline(polyline.clone())).unwrap(),
            Some(2)
        );
        let expected = polyline.segments().map(Geometry::Line).collect::<Vec<_>>();
        let parts = decompose(
            &Geometry::Polyline(polyline),
            Tolerance::DEFAULT,
            usize::MAX,
        )
        .unwrap()
        .unwrap();
        assert!(matches!(parts.report(), (PartKind::Polyline, 2)));
        assert_eq!(
            parts.into_geometries(),
            expected.into_iter().rev().collect::<Vec<_>>()
        );
        for geometry in [
            Geometry::Point(p(0., 0.)),
            Geometry::Line(LineSegment::try_new(p(0., 0.), p(1., 0.), Tolerance::DEFAULT).unwrap()),
        ] {
            assert!(
                decompose(&geometry, Tolerance::DEFAULT, usize::MAX)
                    .unwrap()
                    .is_none()
            );
            assert_eq!(known_output_count(&geometry).unwrap(), None);
        }
    }
}

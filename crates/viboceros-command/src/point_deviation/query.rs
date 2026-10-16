use super::*;
pub(super) struct Target<'a> {
    id: ObjectId,
    geometry: &'a Geometry,
    index: Option<MeshProjectionIndex<'a>>,
}
impl<'a> Target<'a> {
    pub(super) fn new(id: ObjectId, geometry: &'a Geometry) -> Result<Self, CommandError> {
        if geometry.curve_ref().is_none()
            && !matches!(
                geometry,
                Geometry::NurbsSurface(_) | Geometry::Brep(_) | Geometry::Mesh(_)
            )
        {
            return Err(CommandError::Usage(
                "PointDeviation targets must be curves, surfaces, polysurfaces or meshes",
            ));
        }
        Ok(Self {
            id,
            geometry,
            index: if let Geometry::Mesh(m) = geometry {
                Some(MeshProjectionIndex::new(m)?)
            } else {
                None
            },
        })
    }
    pub(super) fn closest(
        &self,
        source: ObjectId,
        point: Point3,
        tolerance: Tolerance,
    ) -> Result<Option<Sample>, CommandError> {
        let mut best: Option<(Point3, Option<Vector3>, bool)> = None;
        let mut admit = |p: Point3, normal: Option<Vector3>, curve: bool| {
            if best
                .as_ref()
                .is_none_or(|b| point.compare_distances(p, b.0).is_lt())
            {
                best = Some((p, normal, curve))
            }
        };
        if let Some(c) = self.geometry.curve_ref() {
            let t = c.closest_parameter(point, tolerance)?;
            let p = c.evaluate(t)?;
            admit(
                p,
                c.evaluate_with_tangent(t)
                    .ok()
                    .map(|s| s.tangent().as_vector()),
                true,
            )
        } else {
            match self.geometry {
                Geometry::NurbsSurface(s) => {
                    let (u, v) = s.closest_parameters(point, tolerance)?;
                    let (p, a, b) = s.evaluate_with_derivatives(u, v)?;
                    admit(p, a.cross(b).ok(), false)
                }
                Geometry::Brep(b) => {
                    for (i, f) in b.faces().iter().enumerate() {
                        if let Some((u, v)) = b.closest_parameters_on_face(i, point, tolerance)? {
                            let (p, a, b) = f.surface().evaluate_with_derivatives(u, v)?;
                            admit(p, a.cross(b).ok(), false)
                        }
                    }
                }
                Geometry::Mesh(_) => {
                    if let Some(index) = &self.index
                        && let Some(location) = index.closest_location(point)?
                    {
                        let [a, b, c] = index
                            .triangle_points(location.triangle_index)
                            .expect("indexed triangle");
                        admit(
                            location.point,
                            a.vector_to(b)?.cross(a.vector_to(c)?).ok(),
                            false,
                        )
                    }
                }
                _ => unreachable!(),
            }
        }
        best.map(|(closest, normal, curve)| {
            let distance = point.distance_to(closest)?;
            let normal_angle = if distance == 0. {
                Some(0.)
            } else {
                normal
                    .and_then(|v| v.normalized_nonzero().ok())
                    .and_then(|n| {
                        closest
                            .direction_to(point)
                            .ok()
                            .and_then(|d| n.as_vector().dot(d.as_vector()).ok())
                    })
                    .map(|dot| {
                        if curve {
                            dot.abs().clamp(0., 1.).asin().to_degrees()
                        } else {
                            dot.abs().clamp(0., 1.).acos().to_degrees()
                        }
                    })
            };
            Ok(Sample {
                source,
                target: self.id,
                point,
                closest,
                distance,
                normal_angle,
            })
        })
        .transpose()
    }
}
pub(super) fn source_points(
    g: &Geometry,
    o: &Options,
    t: Tolerance,
) -> Result<Vec<Point3>, CommandError> {
    let stored_count = match g {
        Geometry::PointCloud(c) => c.points().len(),
        Geometry::Mesh(m) => m.vertices().len(),
        _ => 0,
    };
    if stored_count > MAX_SAMPLES {
        return Err(CommandError::Usage(
            "PointDeviation exceeds 1000000 samples",
        ));
    }
    Ok(match g {
        Geometry::Point(p) => vec![*p],
        Geometry::PointCloud(c) => c.points().to_vec(),
        Geometry::Mesh(m) => m.vertices().to_vec(),
        Geometry::Line(l) => CurveRef::Line(l).divide_by_count(o.curve_divisions, true, t)?,
        Geometry::Polyline(p) => {
            let mut points = CurveRef::Polyline(p).divide_by_count(o.curve_divisions, true, t)?;
            for &p in p.vertices() {
                if !points.contains(&p) {
                    points.push(p)
                }
            }
            points
        }
        Geometry::NurbsSurface(s) => s.tessellate(o.surface_samples, t)?.vertices().to_vec(),
        Geometry::Brep(b) => b.display_mesh(o.surface_samples, t)?.vertices().to_vec(),
        _ => g
            .curve_ref()
            .ok_or(CommandError::Usage(
                "PointDeviation cannot measure block instances",
            ))?
            .divide_by_count(o.curve_divisions, true, t)?,
    })
}

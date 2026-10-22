//! Change a face's UV chart without changing its oriented spatial boundary.
use super::*;

impl Brep {
    /// Exchange selected face charts without moving spatial geometry or
    /// changing oriented incidence. Validate all indices and construct every
    /// replacement before committing; errors leave the B-rep untouched.
    /// This exact change preserves validity by construction and retains the
    /// spatial tables without recomputing their boundary certificates.
    pub fn try_swap_face_uv(&mut self, indices: &[usize]) -> Result<(), GeometryError> {
        if indices.is_empty() {
            return Ok(());
        }
        let mut seen = vec![false; self.faces.len()];
        for &index in indices {
            let slot = seen
                .get_mut(index)
                .ok_or(GeometryError::InvalidBrepTopology {
                    context: "UV swap face index is out of range",
                })?;
            if std::mem::replace(slot, true) {
                return Err(GeometryError::InvalidBrepTopology {
                    context: "UV swap face indices must be unique",
                });
            }
        }
        let replacements = indices
            .iter()
            .map(|&i| Ok((i, self.faces[i].try_swapped_uv()?)))
            .collect::<Result<Vec<_>, GeometryError>>()?;
        self.extrusion = None;
        for (index, face) in replacements {
            self.faces[index] = face;
        }
        Ok(())
    }
}

impl BrepFace {
    /// Exchange the surface's U/V axes and compensate the face sense.
    /// Loop traversal, edge-use direction, UV tolerances and iso labels change
    /// with the chart; spatial edges and vertices require no changes.
    pub fn try_swapped_uv(&self) -> Result<Self, GeometryError> {
        let loops = self
            .loops
            .iter()
            .map(|boundary| {
                let trims = boundary
                    .trims
                    .iter()
                    .rev()
                    .map(|trim| {
                        let controls = trim
                            .curve
                            .control_points()
                            .iter()
                            .map(|c| {
                                WeightedPoint2::try_new(
                                    Point2::try_new(c.point().y(), c.point().x())?,
                                    c.weight(),
                                )
                            })
                            .collect::<Result<Vec<_>, GeometryError>>()?;
                        let curve = NurbsCurve2::try_new_rational(
                            trim.curve.degree(),
                            controls,
                            trim.curve.knots().to_vec(),
                        )?
                        .reversed()?;
                        Ok(BrepTrim {
                            vertices: [trim.vertices[1], trim.vertices[0]],
                            edge: trim.edge,
                            reversed_3d: trim.edge.is_some() && !trim.reversed_3d,
                            curve,
                            trim_type: trim.trim_type,
                            iso: match trim.iso {
                                SurfaceIso::NotIso => SurfaceIso::NotIso,
                                SurfaceIso::InteriorUConstant => SurfaceIso::InteriorVConstant,
                                SurfaceIso::InteriorVConstant => SurfaceIso::InteriorUConstant,
                                SurfaceIso::South => SurfaceIso::West,
                                SurfaceIso::East => SurfaceIso::North,
                                SurfaceIso::North => SurfaceIso::East,
                                SurfaceIso::West => SurfaceIso::South,
                            },
                            tolerance: [trim.tolerance[1], trim.tolerance[0]],
                        })
                    })
                    .collect::<Result<Vec<_>, GeometryError>>()?;
                Ok(BrepLoop {
                    loop_type: boundary.loop_type,
                    trims,
                })
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        Self::try_new(self.surface.try_swapped_uv()?, !self.reversed, loops)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Polyline3;
    #[test]
    fn swapped_face_uv_preserves_spatial_geometry_normals_holes_and_seams() {
        let t = Tolerance::DEFAULT;
        let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
        let frame = Frame3::try_from_directions(
            p(0., 0., 0.),
            Vector3::try_new(1., 0., 0.).unwrap(),
            Vector3::try_new(0., 1., 0.).unwrap(),
            t,
        )
        .unwrap();
        let polygon = |vertices| Polyline3::try_new(vertices, t).unwrap().to_nurbs().unwrap();
        let outer = polygon(vec![
            p(0., 0., 0.),
            p(8., 0., 0.),
            p(8., 6., 0.),
            p(0., 6., 0.),
            p(0., 0., 0.),
        ]);
        let hole = polygon(vec![
            p(2., 2., 0.),
            p(2., 4., 0.),
            p(5., 4., 0.),
            p(5., 2., 0.),
            p(2., 2., 0.),
        ]);
        let planar = Brep::try_planar_face_with_holes(&outer, &[hole], t).unwrap();
        let cylinder =
            Brep::try_surface_face(NurbsSurface::try_cylinder(frame, 2., -3., 5.).unwrap(), t)
                .unwrap();
        let controls = [
            [0., 0., 0.],
            [2., 3., 1.],
            [5., 0., 2.],
            [0., 4., 1.],
            [2., 7., 3.],
            [5., 4., 4.],
        ]
        .into_iter()
        .enumerate()
        .map(|(i, p)| {
            WeightedPoint3::try_new(
                Point3::try_from(p).unwrap(),
                if i % 3 == 1 { 0.5 } else { 1. },
            )
            .unwrap()
        })
        .collect();
        let warped = Brep::try_surface_face(
            NurbsSurface::try_new_rational(
                2,
                1,
                3,
                2,
                controls,
                vec![-4., -4., -4., 2., 2., 2.],
                vec![3., 3., 9., 9.],
            )
            .unwrap(),
            t,
        )
        .unwrap();
        let sphere =
            Brep::try_surface_face(NurbsSurface::try_sphere(frame, 2.).unwrap(), t).unwrap();
        for mut source in [planar, cylinder, warped, sphere] {
            for trim in source
                .faces
                .iter_mut()
                .flat_map(|f| &mut f.loops)
                .flat_map(|l| &mut l.trims)
            {
                trim.tolerance = [1e-12, 2e-12];
            }
            source.validate(t).unwrap();
            for source in [source.clone(), source.reversed()] {
                let mut output = source.clone();
                let vertex_storage = output.vertices.as_ptr();
                let edge_storage = output.edges.as_ptr();
                output
                    .try_swap_face_uv(&(0..source.faces.len()).collect::<Vec<_>>())
                    .unwrap();
                output.validate(t).unwrap();
                assert_eq!(output.vertices.as_ptr(), vertex_storage);
                assert_eq!(output.edges.as_ptr(), edge_storage);
                assert_eq!(source.vertices(), output.vertices());
                assert_eq!(source.edges(), output.edges());
                assert_eq!(source.is_solid(), output.is_solid());
                assert_eq!(source.is_manifold(), output.is_manifold());
                for (a, b) in source.faces().iter().zip(output.faces()) {
                    assert_eq!(a, &b.try_swapped_uv().unwrap());
                    for (u, v) in [(0.2, 0.3), (0.5, 0.5), (0.8, 0.7)] {
                        let u = a.surface().parameter_at_u(u).unwrap();
                        let v = a.surface().parameter_at_v(v).unwrap();
                        assert!(
                            a.surface()
                                .evaluate(u, v)
                                .unwrap()
                                .distance_to(b.surface().evaluate(v, u).unwrap())
                                .unwrap()
                                < 1e-12
                        );
                        assert_eq!(
                            a.contains_parameters(u, v, t).unwrap(),
                            b.contains_parameters(v, u, t).unwrap()
                        );
                        let normal = |f: &BrepFace, u, v| {
                            let n = f.surface().normal_at(u, v).unwrap();
                            if f.is_reversed() { n.opposite() } else { n }
                        };
                        assert!(
                            normal(a, u, v)
                                .as_vector()
                                .dot(normal(b, v, u).as_vector())
                                .unwrap()
                                > 1. - 1e-12
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn invalid_face_uv_indices_are_atomic_and_empty_selection_is_exact() {
        let frame = Frame3::try_from_normal(
            Point3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_new(0., 0., 1.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let source =
            Brep::try_box(frame, [[0., 2.], [0., 3.], [0., 5.]], Tolerance::DEFAULT).unwrap();
        for indices in [
            vec![source.faces.len()],
            vec![0, source.faces.len()],
            vec![0, 0],
            vec![usize::MAX],
        ] {
            let mut output = source.clone();
            assert!(output.try_swap_face_uv(&indices).is_err());
            assert_eq!(output, source);
        }
        let mut output = source.clone();
        output.try_swap_face_uv(&[]).unwrap();
        assert_eq!(output, source);
        output.try_swap_face_uv(&[1, 3, 5]).unwrap();
        output.validate(Tolerance::DEFAULT).unwrap();
        output.try_swap_face_uv(&[1, 3, 5]).unwrap();
        assert_eq!(output, source);
    }
}

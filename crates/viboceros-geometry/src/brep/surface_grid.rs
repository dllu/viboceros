//! Exact tensor partitions with shared edge/vertex topology.

use super::*;

const MAX_GRID_FACES: usize = 4096;

struct GridEdge {
    endpoints: [usize; 2],
    curve: NurbsCurve,
    singular: bool,
    faces: Vec<usize>,
}

impl Brep {
    /// Splits a continuous tensor surface at strictly ordered interior U/V
    /// parameters, retaining native domains and exact shared isocurves.
    /// Natural closed seams and collapsed sides retain their topology. Full-order
    /// interior knots are rejected: a positional jump requires separate shells.
    pub fn try_surface_grid(
        surface: &NurbsSurface,
        cuts_u: &[Real],
        cuts_v: &[Real],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if cuts_u.len() >= MAX_GRID_FACES || cuts_v.len() >= MAX_GRID_FACES {
            return Err(GeometryError::SurfaceGridResourceLimit {
                maximum: MAX_GRID_FACES,
            });
        }
        let u = parameters(surface.domain_u(), cuts_u)?;
        let v = parameters(surface.domain_v(), cuts_v)?;
        let (nu, nv) = (u.len() - 1, v.len() - 1);
        if nu.checked_mul(nv).is_none_or(|n| n > MAX_GRID_FACES) {
            return Err(GeometryError::SurfaceGridResourceLimit {
                maximum: MAX_GRID_FACES,
            });
        }
        for (degree, knots, domain) in [
            (surface.degree_u(), surface.knots_u(), surface.domain_u()),
            (surface.degree_v(), surface.knots_v(), surface.domain_v()),
        ] {
            if knots
                .chunk_by(|a, b| a == b)
                .any(|g| g.len() > degree && g[0] > *domain.start() && g[0] < *domain.end())
            {
                return Err(GeometryError::InvalidBrepTopology {
                    context: "surface grid cannot sew full-order positional breaks",
                });
            }
        }
        let closed_u = surface.is_closed_u()?;
        let closed_v = surface.is_closed_v()?;
        let vertex = |i: usize, j: usize| j * (nu + 1) + i;
        let mut parent = (0..(nu + 1) * (nv + 1)).collect::<Vec<_>>();
        if closed_u {
            for j in 0..=nv {
                unite(&mut parent, vertex(0, j), vertex(nu, j));
            }
        }
        if closed_v {
            for i in 0..=nu {
                unite(&mut parent, vertex(i, 0), vertex(i, nv));
            }
        }
        let mut grid_edges = Vec::<GridEdge>::new();
        let mut edge_map = BTreeMap::new();
        let mut sides = Vec::with_capacity(nu * nv);
        for j in 0..nv {
            for i in 0..nu {
                let face = j * nu + i;
                let mut indices = [0; 4];
                for (side, (axis, a, b)) in [(0, i, j), (1, i + 1, j), (0, i, j + 1), (1, i, j)]
                    .into_iter()
                    .enumerate()
                {
                    let (a, b) = match axis {
                        0 if closed_v && b == nv => (a, 0),
                        1 if closed_u && a == nu => (0, b),
                        _ => (a, b),
                    };
                    let key = (axis, a, b);
                    let index = if let Some(&index) = edge_map.get(&key) {
                        index
                    } else {
                        let (curve, endpoints) = if axis == 0 {
                            (
                                surface.isocurve_u(v[b])?.try_trimmed(u[a]..=u[a + 1])?,
                                [vertex(a, b), vertex(a + 1, b)],
                            )
                        } else {
                            (
                                surface.isocurve_v(u[a])?.try_trimmed(v[b]..=v[b + 1])?,
                                [vertex(a, b), vertex(a, b + 1)],
                            )
                        };
                        let first = curve.control_points()[0].point();
                        let singular = curve.control_points().iter().all(|c| c.point() == first);
                        if singular {
                            unite(&mut parent, endpoints[0], endpoints[1]);
                        }
                        let index = grid_edges.len();
                        grid_edges.push(GridEdge {
                            endpoints,
                            curve,
                            singular,
                            faces: Vec::new(),
                        });
                        edge_map.insert(key, index);
                        index
                    };
                    grid_edges[index].faces.push(face);
                    indices[side] = index;
                }
                sides.push(indices);
            }
        }
        let mut vertices = Vec::new();
        let mut vertex_map = BTreeMap::new();
        let mut indices = Vec::with_capacity(parent.len());
        for k in 0..parent.len() {
            let root = root(&parent, k);
            let index = if let Some(&index) = vertex_map.get(&root) {
                index
            } else {
                let index = vertices.len();
                vertices.push(BrepVertex::try_new(
                    surface.evaluate(u[root % (nu + 1)], v[root / (nu + 1)])?,
                    tolerance.absolute(),
                )?);
                vertex_map.insert(root, index);
                index
            };
            indices.push(index);
        }
        let mut edges = Vec::new();
        let mut edge_indices = Vec::new();
        for edge in &grid_edges {
            if edge.singular {
                edge_indices.push(None);
            } else {
                edge_indices.push(Some(edges.len()));
                edges.push(BrepEdge::try_new(
                    edge.endpoints.map(|i| indices[i]),
                    edge.curve.clone(),
                    tolerance.absolute(),
                )?);
            }
        }
        let mut faces = Vec::with_capacity(nu * nv);
        for j in 0..nv {
            for i in 0..nu {
                let corners = [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)];
                let mut trims = Vec::with_capacity(4);
                for (side, &edge_index) in sides[j * nu + i].iter().enumerate() {
                    let edge = &grid_edges[edge_index];
                    let start = corners[side];
                    let end = corners[(side + 1) % 4];
                    let kind = if edge.singular {
                        BrepTrimType::Singular
                    } else if edge.faces.len() == 1 {
                        BrepTrimType::Boundary
                    } else if edge.faces[0] == edge.faces[1] {
                        BrepTrimType::Seam
                    } else {
                        BrepTrimType::Mated
                    };
                    trims.push(BrepTrim::try_new(
                        [
                            indices[vertex(start.0, start.1)],
                            indices[vertex(end.0, end.1)],
                        ],
                        edge_indices[edge_index],
                        !edge.singular && side >= 2,
                        NurbsCurve2::try_line(
                            Point2::try_new(u[start.0], v[start.1])?,
                            Point2::try_new(u[end.0], v[end.1])?,
                        )?,
                        kind,
                        [
                            SurfaceIso::South,
                            SurfaceIso::East,
                            SurfaceIso::North,
                            SurfaceIso::West,
                        ][side],
                        [0.0, 0.0],
                    )?);
                }
                faces.push(BrepFace::try_new(
                    surface.try_trimmed(u[i]..=u[i + 1], v[j]..=v[j + 1])?,
                    false,
                    vec![BrepLoop::try_new(BrepLoopType::Outer, trims)?],
                )?);
            }
        }
        Self::try_new(vertices, edges, faces, tolerance)
    }
}

fn parameters(domain: RangeInclusive<Real>, cuts: &[Real]) -> Result<Vec<Real>, GeometryError> {
    require_finite(cuts.iter().copied(), "surface grid cuts")?;
    let mut result = vec![*domain.start()];
    for &cut in cuts {
        if cut <= *result.last().unwrap() || cut >= *domain.end() {
            return Err(GeometryError::InvalidBrepTopology {
                context: "surface grid cuts must be strictly ordered and interior",
            });
        }
        result.push(cut);
    }
    result.push(*domain.end());
    Ok(result)
}
fn root(parent: &[usize], mut i: usize) -> usize {
    while parent[i] != i {
        i = parent[i];
    }
    i
}
fn unite(parent: &mut [usize], a: usize, b: usize) {
    let a = root(parent, a);
    let b = root(parent, b);
    parent[a.max(b)] = a.min(b);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame() -> Frame3 {
        Frame3::try_from_directions(
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
    }
    #[test]
    fn partitioned_sphere_sews_seams_and_poles_without_welding_unrelated_points() {
        let s = NurbsSurface::try_sphere(frame(), 1.0).unwrap();
        let u = (1..4)
            .map(|i| s.parameter_at_u(i as Real / 4.0).unwrap())
            .collect::<Vec<_>>();
        let b = Brep::try_surface_grid(
            &s,
            &u,
            &[s.parameter_at_v(0.5).unwrap()],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(b.faces().len(), 8);
        assert!(b.is_solid());
        assert_eq!(b.vertices().len(), 6);
        assert_eq!(b.edges().len(), 12);
        let m = b
            .polygon_mesh(0.0, false, false, Tolerance::DEFAULT)
            .unwrap();
        assert!(m.topology().is_closed());
        assert!(m.topology().is_manifold());
        assert!(
            (b.signed_volume(Tolerance::DEFAULT).unwrap() - 4.0 * std::f64::consts::PI / 3.0).abs()
                < 1e-8
        );
    }
    #[test]
    fn invalid_grid_cuts_do_not_change_source_geometry() {
        let s = NurbsSurface::try_sphere(frame(), 1.0).unwrap();
        let end = *s.domain_u().end();
        for cuts in [
            vec![end],
            vec![-1.0],
            vec![1.0, 1.0],
            vec![2.0, 1.0],
            vec![Real::NAN],
        ] {
            assert!(Brep::try_surface_grid(&s, &cuts, &[], Tolerance::DEFAULT).is_err());
        }
        assert!(matches!(
            Brep::try_surface_grid(&s, &vec![0.0; MAX_GRID_FACES], &[], Tolerance::DEFAULT),
            Err(GeometryError::SurfaceGridResourceLimit { .. })
        ));
    }

    #[test]
    fn unsplit_periodic_sphere_retains_its_exact_surface_and_singular_trims() {
        let s = NurbsSurface::try_sphere(frame(), 1.0).unwrap();
        let b = Brep::try_surface_grid(&s, &[], &[], Tolerance::DEFAULT).unwrap();
        assert_eq!(b.faces()[0].surface(), &s);
        assert_eq!(
            (b.vertices().len(), b.edges().len(), b.faces().len()),
            (2, 1, 1)
        );
        assert!(b.is_solid());
    }

    #[test]
    fn open_tensor_grid_shares_only_parametrically_adjacent_boundaries() {
        let s = NurbsSurface::try_new(
            1,
            1,
            2,
            2,
            [
                [0.0, 0.0, 0.0],
                [3.0, 0.0, 0.0],
                [0.0, 2.0, 0.0],
                [3.0, 2.0, 1.0],
            ]
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
            vec![0.0, 0.0, 3.0, 3.0],
            vec![0.0, 0.0, 2.0, 2.0],
        )
        .unwrap();
        let b = Brep::try_surface_grid(&s, &[1.0, 2.0], &[1.0], Tolerance::DEFAULT).unwrap();
        assert_eq!(
            (b.vertices().len(), b.edges().len(), b.faces().len()),
            (12, 17, 6)
        );
        assert!(!b.is_solid());
        let mesh = b
            .polygon_mesh(0.0, false, false, Tolerance::DEFAULT)
            .unwrap();
        assert!(mesh.topology().is_manifold());
        assert!(!mesh.topology().is_closed());
        for face in b.faces() {
            let patch = face.surface();
            for i in 0..=4 {
                for j in 0..=4 {
                    let u = patch.parameter_at_u(i as Real / 4.0).unwrap();
                    let v = patch.parameter_at_v(j as Real / 4.0).unwrap();
                    assert!(
                        patch
                            .evaluate(u, v)
                            .unwrap()
                            .distance_to(s.evaluate(u, v).unwrap())
                            .unwrap()
                            < 1e-12
                    );
                }
            }
        }
    }
}

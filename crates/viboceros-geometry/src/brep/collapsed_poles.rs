//! Promote certified pointlike naked edges to singular trim records.
use super::*;

impl Brep {
    /// Restricts selected pole-face supports to their active UV boundaries and
    /// represents the named pointlike edges as singular trims. Existing UV
    /// paths and every noncollapsed 3D edge remain unchanged.
    pub fn try_normalize_pole_edges(
        &self,
        pole_edges: &[usize],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let selected = pole_edges.iter().copied().collect::<BTreeSet<_>>();
        if selected.is_empty()
            || selected.len() != pole_edges.len()
            || selected.last().is_some_and(|&i| i >= self.edges.len())
        {
            return invalid("pole normalization needs distinct existing edges");
        }
        let mut support_faces = BTreeSet::new();
        let mut owner_counts = vec![0usize; self.edges.len()];
        for usage in self.trim_uses() {
            if let Some(e) = usage.trim.edge.filter(|e| selected.contains(e)) {
                owner_counts[e] += 1;
                support_faces.insert(usage.face);
            }
        }
        let mut poles = BTreeMap::new();
        let mut roots = (0..self.vertices.len()).collect::<Vec<_>>();
        for &e in &selected {
            if owner_counts[e] != 1 {
                return invalid("pole normalization requires a singly owned edge");
            }
            let edge = &self.edges[e];
            let controls = edge.curve.control_points();
            let point = controls[0].point();
            let scale = point
                .to_array()
                .into_iter()
                .map(Real::abs)
                .fold(1., Real::max);
            let epsilon = scale * (512. * Real::EPSILON);
            let sign = controls[0].weight().is_sign_positive();
            for p in controls {
                if p.weight().is_sign_positive() != sign || p.point().distance_to(point)? > epsilon
                {
                    return invalid("named pole edge is not collapsed to world roundoff");
                }
            }
            poles.insert(e, (point, epsilon));
            let [a, b] = edge.vertices.map(|i| root(&mut roots, i));
            roots[a.max(b)] = a.min(b);
        }

        let mut result = self.try_shrunk_surface_faces(
            &support_faces.into_iter().collect::<Vec<_>>(),
            BrepSurfaceShrinkMode::ToEdge,
            tolerance,
        )?;
        let mut vertices: Vec<BrepVertex> = Vec::new();
        let mut vertex_map = vec![0; self.vertices.len()];
        for (i, v) in self.vertices.iter().enumerate() {
            let r = root(&mut roots, i);
            if r == i {
                vertex_map[i] = vertices.len();
                vertices.push(*v);
            } else {
                let first = &mut vertices[vertex_map[r]];
                let gap = v.point.distance_to(first.point)?;
                if gap > tolerance.absolute() {
                    return invalid("collapsed pole vertices exceed document tolerance");
                }
                first.tolerance = first.tolerance.max(v.tolerance + gap);
                vertex_map[i] = vertex_map[r];
            }
        }
        let mut edges = Vec::new();
        let mut edge_map = vec![None; self.edges.len()];
        for (i, mut edge) in result.edges.into_iter().enumerate() {
            if !selected.contains(&i) {
                edge_map[i] = Some(edges.len());
                edge.vertices = edge.vertices.map(|v| vertex_map[v]);
                edges.push(edge);
            }
        }
        for face in &mut result.faces {
            for trim in face.loops.iter_mut().flat_map(|l| &mut l.trims) {
                trim.vertices = trim.vertices.map(|v| vertex_map[v]);
                let Some(e) = trim.edge else {
                    continue;
                };
                trim.edge = edge_map[e];
                if let Some(&(point, epsilon)) = poles.get(&e) {
                    let iso = pole_side(&face.surface, &trim.curve).ok_or(
                        GeometryError::InvalidBrepTopology {
                            context: "collapsed pole is outside the shrunken surface boundaries",
                        },
                    )?;
                    let reference = NurbsCurve::try_new_rational(
                        1,
                        vec![WeightedPoint3::try_new(point, 1.)?; 2],
                        vec![0., 0., 1., 1.],
                    )?;
                    if face
                        .surface
                        .parameter_curve_deviation_bound(&trim.curve, &reference, epsilon)?
                        .is_none()
                    {
                        return invalid("pole trim image cannot be certified at world roundoff");
                    }
                    trim.trim_type = BrepTrimType::Singular;
                    trim.reversed_3d = false;
                    trim.iso = iso;
                }
            }
        }
        Brep::try_new(vertices, edges, result.faces, tolerance)
    }
}

fn root(roots: &mut [usize], mut i: usize) -> usize {
    while roots[i] != i {
        roots[i] = roots[roots[i]];
        i = roots[i];
    }
    i
}

fn pole_side(surface: &NurbsSurface, curve: &NurbsCurve2) -> Option<SurfaceIso> {
    for (u, domain, lower, upper) in [
        (true, surface.domain_u(), SurfaceIso::West, SurfaceIso::East),
        (
            false,
            surface.domain_v(),
            SurfaceIso::South,
            SurfaceIso::North,
        ),
    ] {
        for (bound, side) in [(*domain.start(), lower), (*domain.end(), upper)] {
            let epsilon =
                bound.abs().max((domain.end() - domain.start()).abs()) * (128. * Real::EPSILON);
            if curve.control_points().iter().all(|q| {
                let p = q.point();
                let value = if u { p.x() } else { p.y() };
                (value - bound).abs() <= epsilon
            }) {
                return Some(side);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_pole_edge_is_recoverable_without_changing_the_other_trims() {
        let tol = Tolerance::DEFAULT;
        let frame = Frame3::try_from_normal(
            Point3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_new(0., 0., 1.).unwrap(),
            tol,
        )
        .unwrap();
        let surface = NurbsSurface::try_sphere(frame, 2.).unwrap();
        let original = Brep::try_surface_face(surface, tol).unwrap();
        let mut legacy = original.clone();
        let trim = &mut legacy.faces[0].loops[0].trims[0];
        assert_eq!(trim.trim_type, BrepTrimType::Singular);
        let vertex = trim.vertices[0];
        let point = legacy.vertices[vertex].point;
        let curve = NurbsCurve::try_new_rational(
            1,
            vec![WeightedPoint3::try_new(point, 1.).unwrap(); 2],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        let index = legacy.edges.len();
        legacy
            .edges
            .push(BrepEdge::try_new([vertex, vertex], curve, 0.).unwrap());
        trim.edge = Some(index);
        trim.trim_type = BrepTrimType::Boundary;
        assert!(legacy.try_normalize_pole_edges(&[0], tol).is_err());
        assert_eq!(
            legacy.try_normalize_pole_edges(&[index], tol).unwrap(),
            original
        );
    }
}

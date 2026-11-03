//! Exact miter corners and native collinear-junction policy for line segments.
use super::*;
pub(super) fn supported(brep: &Brep, boundary: &BrepLoop) -> bool {
    boundary.trims.len() >= 3
        && boundary.trims.iter().all(|t| {
            matches!(t.trim_type, BrepTrimType::Boundary | BrepTrimType::Mated)
                && t.edge.is_some_and(|e| {
                    brep.edges[e].curve.degree() == 1
                        && brep.edges[e].curve.control_points().len() == 2
                })
                && t.curve.degree() == 1
                && t.curve.control_points().len() == 2
        })
}
impl Brep {
    pub(super) fn inset_polygon_boundary(
        &mut self,
        face: &BrepFace,
        boundary: &BrepLoop,
        normal: crate::UnitVector3,
        distance: Real,
        crease: bool,
        tolerance: Tolerance,
    ) -> Result<Boundary, GeometryError> {
        let old = &boundary.trims;
        let mut borders = Vec::new();
        let mut retained = Vec::new();
        let offset_points = old
            .iter()
            .enumerate()
            .map(|(j, trim)| {
                let previous = &old[(j + old.len() - 1) % old.len()];
                let p = self.vertices[trim.vertices[0]].point;
                let incoming = self.vertices[previous.vertices[0]]
                    .point
                    .vector_to(p)?
                    .normalized_nonzero()?
                    .as_vector();
                let outgoing = p
                    .vector_to(self.vertices[trim.vertices[1]].point)?
                    .normalized_nonzero()?
                    .as_vector();
                let denominator = 1. + incoming.dot(outgoing)?;
                if denominator <= 64. * Real::EPSILON {
                    return Err(invalid("inset corner reverses direction"));
                }
                if incoming.cross(outgoing)?.length()? > 64. * Real::EPSILON {
                    retained.push(j);
                }
                let sum = Vector3::try_from(std::array::from_fn(|k| {
                    incoming.to_array()[k] + outgoing.to_array()[k]
                }))?;
                let direction = normal.as_vector().cross(sum)?;
                p.translated(direction.scaled(distance / denominator)?)
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        if retained.len() < 3 {
            return Err(invalid("inset boundary has fewer than three corners"));
        }
        // Native offsets simplify collinear source junctions. The crease
        // connector at such an old vertex goes to the following retained
        // offset corner; the following strip can therefore be triangular.
        let offset_points = retained
            .iter()
            .map(|&j| offset_points[j])
            .collect::<Vec<_>>();
        let mut next = 0;
        let mut cursor = retained.len();
        let mut mapped = vec![0; old.len()];
        for j in (0..old.len()).rev() {
            if cursor > 0 && retained[cursor - 1] == j {
                cursor -= 1;
                next = cursor;
            }
            mapped[j] = next;
        }
        let vertex_start = self.vertices.len();
        for &p in &offset_points {
            self.vertices.push(BrepVertex::try_new(p, 0.)?);
        }
        let mut inset = Vec::with_capacity(offset_points.len());
        for j in 0..offset_points.len() {
            let k = (j + 1) % offset_points.len();
            inset.push(self.inset_line_trim(
                &face.surface,
                [vertex_start + j, vertex_start + k],
                [offset_points[j], offset_points[k]],
                tolerance,
            )?);
        }
        if crease {
            let mut connectors = Vec::with_capacity(old.len());
            for (j, trim) in old.iter().enumerate() {
                connectors.push(self.inset_line_trim(
                    &face.surface,
                    [trim.vertices[0], vertex_start + mapped[j]],
                    [
                        self.vertices[trim.vertices[0]].point,
                        offset_points[mapped[j]],
                    ],
                    tolerance,
                )?);
            }
            for j in 0..old.len() {
                let k = (j + 1) % old.len();
                let mut boundary = vec![old[j].clone(), connectors[k].clone()];
                if mapped[j] != mapped[k] {
                    boundary.push(reverse_trim(&inset[mapped[j]])?);
                }
                boundary.push(reverse_trim(&connectors[j])?);
                borders.push(BrepFace::try_from_polygon_boundaries(
                    face.surface.clone(),
                    face.reversed,
                    vec![boundary],
                )?);
            }
        } else {
            let reversed = inset
                .iter()
                .rev()
                .map(reverse_trim)
                .collect::<Result<Vec<_>, _>>()?;
            borders.push(BrepFace::try_from_polygon_boundaries(
                face.surface.clone(),
                face.reversed,
                vec![old.clone(), reversed],
            )?);
        }
        Ok(Boundary { inset, borders })
    }
}

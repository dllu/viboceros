use super::*;

mod boundary;

#[cfg(test)]
mod tests;

impl Brep {
    pub(super) fn validate(&self, tolerance: Tolerance) -> Result<(), GeometryError> {
        for edge in &self.edges {
            let allowed = tolerance.absolute().max(edge.tolerance);
            boundary::continuous(&edge.curve, |delta| {
                delta[0].hypot(delta[1]).hypot(delta[2]) <= allowed
            })?;
            if edge
                .vertices
                .iter()
                .any(|vertex| *vertex >= self.vertices.len())
            {
                return invalid("an edge references a missing vertex");
            }
            let domain = edge.curve.domain();
            let endpoints = [
                edge.curve.evaluate(*domain.start())?,
                edge.curve.evaluate(*domain.end())?,
            ];
            for (end, endpoint) in endpoints.into_iter().enumerate() {
                let vertex = self.vertices[edge.vertices[end]];
                let allowed = tolerance
                    .absolute()
                    .max(vertex.tolerance)
                    .max(edge.tolerance);
                if endpoint.distance_to(vertex.point)? > allowed {
                    return invalid("an edge-curve endpoint misses its vertex");
                }
            }
        }

        for face in &self.faces {
            for face_loop in &face.loops {
                self.validate_loop(face, face_loop, tolerance)?;
            }
        }

        let uses = self.trim_uses();
        let mut edge_uses = vec![Vec::new(); self.edges.len()];
        for trim_use in &uses {
            if let Some(edge) = trim_use.trim.edge {
                edge_uses[edge].push(trim_use);
            }
        }
        for edge_uses in edge_uses {
            if edge_uses.is_empty() {
                return invalid("every B-rep edge must be used by a trim");
            }
            for trim_use in &edge_uses {
                let same_loop_uses = edge_uses
                    .iter()
                    .filter(|other| {
                        trim_use.face == other.face && trim_use.face_loop == other.face_loop
                    })
                    .count();
                let expected = if edge_uses.len() == 1 {
                    BrepTrimType::Boundary
                } else if same_loop_uses >= 2 {
                    BrepTrimType::Seam
                } else {
                    BrepTrimType::Mated
                };
                if trim_use.trim.trim_type != expected {
                    return invalid("a trim type disagrees with its edge-use topology");
                }
            }
        }
        Ok(())
    }

    fn validate_loop(
        &self,
        face: &BrepFace,
        face_loop: &BrepLoop,
        tolerance: Tolerance,
    ) -> Result<(), GeometryError> {
        for (index, trim) in face_loop.trims.iter().enumerate() {
            if trim
                .vertices
                .iter()
                .any(|vertex| *vertex >= self.vertices.len())
            {
                return invalid("a trim references a missing vertex");
            }
            if let Some(edge_index) = trim.edge {
                let Some(edge) = self.edges.get(edge_index) else {
                    return invalid("a trim references a missing edge");
                };
                let expected = if trim.reversed_3d {
                    [edge.vertices[1], edge.vertices[0]]
                } else {
                    edge.vertices
                };
                if trim.vertices != expected {
                    return invalid("a trim's vertex direction disagrees with its 3D edge");
                }
            }

            let next = &face_loop.trims[(index + 1) % face_loop.trims.len()];
            if trim.vertices[1] != next.vertices[0] {
                return invalid("a trim loop is not topologically closed");
            }
            let end = trim.curve.end_point()?;
            let next_start = next.curve.start_point()?;
            let parameter_tolerance = [
                tolerance
                    .absolute()
                    .max(trim.tolerance[0])
                    .max(next.tolerance[0]),
                tolerance
                    .absolute()
                    .max(trim.tolerance[1])
                    .max(next.tolerance[1]),
            ];
            if !parameter_points_near(end, next_start, parameter_tolerance) {
                return invalid("adjacent p-curves do not meet in parameter space");
            }

            let parameter_ends = [trim.curve.start_point()?, end];
            for (end_index, parameter) in parameter_ends.into_iter().enumerate() {
                let surface_point = face.surface.evaluate(parameter.x(), parameter.y())?;
                let vertex = self.vertices[trim.vertices[end_index]];
                let allowed = tolerance.absolute().max(vertex.tolerance);
                if surface_point.distance_to(vertex.point)? > allowed {
                    return invalid("a p-curve endpoint misses its model-space vertex");
                }
            }
            validate_iso(face, trim, parameter_tolerance)?;
            boundary::validate(self, face, trim, tolerance)?;
        }

        let signed_area = sampled_loop_signed_area(face_loop)?;
        let orientation_is_valid = match face_loop.loop_type {
            BrepLoopType::Outer => signed_area > 0.0,
            BrepLoopType::Inner => signed_area < 0.0,
        };
        if !orientation_is_valid {
            return invalid("outer p-loops must be counterclockwise and inner p-loops clockwise");
        }
        Ok(())
    }
}

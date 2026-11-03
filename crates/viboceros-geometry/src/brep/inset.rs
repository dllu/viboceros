//! Planar polygon insets retaining the supporting surfaces and shared edges.
use super::*;

#[cfg(test)]
mod tests;

const MAX_CORNERS: usize = 100_000;

fn invalid(context: &'static str) -> GeometryError {
    GeometryError::InvalidBrepTopology { context }
}

impl Brep {
    /// Individually inset straight-sided faces on affine planar surfaces.
    /// Outer boundaries shrink and holes expand by a positive model distance.
    /// With `crease_corners`, each old edge and its offset bound a new strip;
    /// otherwise each boundary receives one annular face. Original surfaces,
    /// face senses, vertices, edges and unselected faces are retained exactly.
    /// Selected face slots hold the new caps; border faces are appended.
    ///
    /// Curved edges, seams, poles, non-affine charts, collapsed/crossing offsets
    /// and topology changes at the medial axis return errors without mutation.
    /// The source must use one straight segment per trim; kinked polycurve
    /// edges can first be partitioned with `try_split_kinky_edges`. Admission
    /// is bounded to 100,000 selected corners, 16 million boundary pairs and
    /// a conservative 64 MiB output-storage estimate, checked before cloning.
    pub fn try_inset_planar_faces(
        &self,
        selected: &[usize],
        distance: Real,
        crease_corners: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if !distance.is_finite() || distance <= 0. || selected.is_empty() {
            return Err(invalid(
                "planar inset requires faces and a positive finite distance",
            ));
        }
        let mut indices = BTreeSet::new();
        let mut corners = 0usize;
        let mut work = 0usize;
        for &i in selected {
            let face = self
                .faces
                .get(i)
                .ok_or_else(|| invalid("missing inset face"))?;
            if !indices.insert(i) {
                return Err(invalid("duplicate inset face"));
            }
            let n = face.loops.iter().map(|l| l.trims.len()).sum::<usize>();
            corners = corners.saturating_add(n);
            work = work.saturating_add(n.saturating_mul(n));
            if corners > MAX_CORNERS || work > 16_000_000 {
                return Err(invalid("planar inset exceeds its boundary work limit"));
            }
        }
        self.check_inset_storage(&indices, crease_corners)?;
        let mut output = self.clone();
        for i in indices {
            output.inset_polygon_face(i, distance, crease_corners, tolerance)?;
        }
        Self::try_new(output.vertices, output.edges, output.faces, tolerance)
    }

    fn check_inset_storage(
        &self,
        selected: &BTreeSet<usize>,
        crease: bool,
    ) -> Result<(), GeometryError> {
        // Border faces retain the complete supporting control net. Charge
        // those copies before construction, including highly refined planes.
        let mut bytes = self.vertices.len().saturating_mul(32);
        for edge in &self.edges {
            bytes = bytes
                .saturating_add(256)
                .saturating_add(edge.curve.control_points().len().saturating_mul(32))
                .saturating_add(edge.curve.knots().len().saturating_mul(8));
        }
        for (i, face) in self.faces.iter().enumerate() {
            let surface = face
                .surface
                .control_points()
                .len()
                .saturating_mul(32)
                .saturating_add(face.surface.knots_u().len().saturating_mul(8))
                .saturating_add(face.surface.knots_v().len().saturating_mul(8))
                .saturating_add(256);
            bytes = bytes.saturating_add(surface);
            let mut count = 0usize;
            for boundary in &face.loops {
                bytes = bytes.saturating_add(128);
                count = count.saturating_add(boundary.trims.len());
                for trim in &boundary.trims {
                    bytes = bytes
                        .saturating_add(256)
                        .saturating_add(trim.curve.control_points().len().saturating_mul(24))
                        .saturating_add(trim.curve.knots().len().saturating_mul(8));
                }
            }
            if selected.contains(&i) {
                let borders = if crease { count } else { face.loops.len() };
                bytes = bytes
                    .saturating_add(surface.saturating_mul(borders))
                    .saturating_add(count.saturating_mul(2048));
            }
            if bytes > 64 * 1024 * 1024 {
                return Err(invalid("planar inset exceeds its output-storage budget"));
            }
        }
        Ok(())
    }

    fn inset_polygon_face(
        &mut self,
        index: usize,
        distance: Real,
        crease: bool,
        tolerance: Tolerance,
    ) -> Result<(), GeometryError> {
        let face = self.faces[index].clone();
        let [origin, east, _, north] = face.surface.try_affine_patch_corners(tolerance)?;
        let normal = origin
            .vector_to(east)?
            .cross(origin.vector_to(north)?)?
            .normalized_nonzero()?;
        let mut caps = Vec::with_capacity(face.loops.len());
        let mut borders = Vec::new();
        for boundary in &face.loops {
            let old = &boundary.trims;
            if old.len() < 3 {
                return Err(invalid("planar inset requires polygon boundaries"));
            }
            for trim in old {
                let edge = trim
                    .edge
                    .ok_or_else(|| invalid("inset does not support poles"))?;
                let curve = &self.edges[edge].curve;
                if matches!(trim.trim_type, BrepTrimType::Seam | BrepTrimType::Singular)
                    || curve.degree() != 1
                    || curve.control_points().len() != 2
                    || trim.curve.degree() != 1
                    || trim.curve.control_points().len() != 2
                {
                    return Err(invalid(
                        "planar inset requires individual straight non-seam trims",
                    ));
                }
            }
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
            caps.push(inset);
        }
        self.faces[index] =
            BrepFace::try_from_polygon_boundaries(face.surface, face.reversed, caps)?;
        self.faces.extend(borders);
        Ok(())
    }

    fn inset_line_trim(
        &mut self,
        surface: &NurbsSurface,
        vertices: [usize; 2],
        points: [Point3; 2],
        tolerance: Tolerance,
    ) -> Result<BrepTrim, GeometryError> {
        if points[0].distance_to(points[1])? <= tolerance.absolute() {
            return Err(invalid("inset collapses a boundary edge"));
        }
        let curve = NurbsCurve::try_new(1, points.to_vec(), vec![0., 0., 1., 1.])?;
        let uv = surface.try_pullback_affine_curve(&curve, tolerance)?;
        let edge = self.edges.len();
        self.edges.push(BrepEdge::try_new(vertices, curve, 0.)?);
        let iso = trim_iso::classify(&uv, surface);
        BrepTrim::try_new(
            vertices,
            Some(edge),
            false,
            uv,
            BrepTrimType::Mated,
            iso,
            [0.; 2],
        )
    }
}

fn reverse_trim(trim: &BrepTrim) -> Result<BrepTrim, GeometryError> {
    let mut result = trim.clone();
    result.vertices.swap(0, 1);
    result.reversed_3d = !result.reversed_3d;
    result.curve = result.curve.reversed()?;
    Ok(result)
}

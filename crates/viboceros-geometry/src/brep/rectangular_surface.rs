//! Surface-preserving natural and rectangular face construction.
use super::*;

#[cfg(test)]
mod tests;

impl Brep {
    /// Wraps a NURBS surface as one exact natural-domain face.
    pub fn try_surface_face(
        surface: NurbsSurface,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let u = surface.domain_u();
        let v = surface.domain_v();
        Self::try_rectangular_surface_face(surface, u, v, tolerance)
    }

    /// Builds one exact face whose rectangular trim lies in the supplied
    /// subdomain while retaining the complete underlying NURBS surface.
    ///
    /// Closed directions share one seam edge between their two trims, and
    /// collapsed sides become singular trims without a 3D edge. Interior
    /// constant-U and constant-V trims retain their OpenNURBS isoparametric
    /// classes. The surface and UV trims keep their native coordinates;
    /// generated spatial edges use lossless local parameter domains.
    pub fn try_rectangular_surface_face(
        surface: NurbsSurface,
        u: RangeInclusive<Real>,
        v: RangeInclusive<Real>,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Self::try_rectangular_surface_face_with_orientation(surface, u, v, false, tolerance)
    }

    /// Builds the same rectangular face while explicitly preserving its
    /// orientation relative to the underlying surface.
    pub fn try_rectangular_surface_face_with_orientation(
        surface: NurbsSurface,
        u: RangeInclusive<Real>,
        v: RangeInclusive<Real>,
        reversed: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        require_finite(
            [*u.start(), *u.end(), *v.start(), *v.end()],
            "rectangular surface-face trim bounds",
        )?;
        // Validate native intervals without allocating an unused tensor trim.
        crate::parameter::check_trim_interval(&u, surface.domain_u())?;
        crate::parameter::check_trim_interval(&v, surface.domain_v())?;
        let bounds = [[*u.start(), *u.end()], [*v.start(), *v.end()]];
        let frame = surface.local_parameter_frame(
            [[bounds[0][0], bounds[1][0]], [bounds[0][1], bounds[1][1]]].into_iter(),
        )?;
        let local = &frame.surface;
        let local_bounds: [[Real; 2]; 2] =
            std::array::from_fn(|axis| bounds[axis].map(|p| p - frame.origin[axis]));
        let corner_points = [
            local.evaluate(local_bounds[0][0], local_bounds[1][0])?,
            local.evaluate(local_bounds[0][1], local_bounds[1][0])?,
            local.evaluate(local_bounds[0][1], local_bounds[1][1])?,
            local.evaluate(local_bounds[0][0], local_bounds[1][1])?,
        ];
        let side_curves = [
            local
                .isocurve_u(local_bounds[1][0])?
                .try_trimmed(local_bounds[0][0]..=local_bounds[0][1])?,
            local
                .isocurve_v(local_bounds[0][1])?
                .try_trimmed(local_bounds[1][0]..=local_bounds[1][1])?,
            local
                .isocurve_u(local_bounds[1][1])?
                .try_trimmed(local_bounds[0][0]..=local_bounds[0][1])?
                .reversed()?,
            local
                .isocurve_v(local_bounds[0][0])?
                .try_trimmed(local_bounds[1][0]..=local_bounds[1][1])?
                .reversed()?,
        ];
        let singular = side_curves.each_ref().map(|curve| {
            let first = curve.control_points()[0].point();
            curve
                .control_points()
                .iter()
                .all(|control| control.point() == first)
        });

        // Join corner records only where the intervening topological side
        // closes or collapses. Coincident points on unrelated sides remain
        // distinct vertices, as required at self-intersections.
        let mut corner_groups = [0, 1, 2, 3];
        for side in 0..4 {
            if corner_points[side].distance_to(corner_points[(side + 1) % 4])?
                <= tolerance.absolute()
            {
                let first = corner_groups[side];
                let second = corner_groups[(side + 1) % 4];
                for group in &mut corner_groups {
                    if *group == second {
                        *group = first;
                    }
                }
            }
        }
        let surface_u = surface.domain_u();
        let surface_v = surface.domain_v();
        let closed_u = bounds[0][0] == *surface_u.start()
            && bounds[0][1] == *surface_u.end()
            && local.is_closed_u()?;
        let closed_v = bounds[1][0] == *surface_v.start()
            && bounds[1][1] == *surface_v.end()
            && local.is_closed_v()?;
        let seam_sides = [closed_v, closed_u, closed_v, closed_u];

        let mut group_vertices = [usize::MAX; 4];
        let mut corner_vertices = [usize::MAX; 4];
        let mut vertices = Vec::new();
        for corner in 0..4 {
            let group = corner_groups[corner];
            if group_vertices[group] == usize::MAX {
                group_vertices[group] = vertices.len();
                vertices.push(BrepVertex::try_new(corner_points[corner], 0.0)?);
            }
            corner_vertices[corner] = group_vertices[group];
        }

        let mut edge_indices = [None; 4];
        let mut reversed_3d = [false; 4];
        let mut edges = Vec::new();
        for side in 0..4 {
            if singular[side] {
                continue;
            }
            let paired_side = match side {
                2 if closed_v && !singular[0] => Some(0),
                3 if closed_u && !singular[1] => Some(1),
                _ => None,
            };
            if let Some(paired_side) = paired_side {
                edge_indices[side] = edge_indices[paired_side];
                reversed_3d[side] = true;
                continue;
            }
            edge_indices[side] = Some(edges.len());
            edges.push(BrepEdge::try_new(
                [corner_vertices[side], corner_vertices[(side + 1) % 4]],
                side_curves[side].clone(),
                0.0,
            )?);
        }
        let iso = [
            if bounds[1][0] == *surface_v.start() {
                SurfaceIso::South
            } else {
                SurfaceIso::InteriorVConstant
            },
            if bounds[0][1] == *surface_u.end() {
                SurfaceIso::East
            } else {
                SurfaceIso::InteriorUConstant
            },
            if bounds[1][1] == *surface_v.end() {
                SurfaceIso::North
            } else {
                SurfaceIso::InteriorVConstant
            },
            if bounds[0][0] == *surface_u.start() {
                SurfaceIso::West
            } else {
                SurfaceIso::InteriorUConstant
            },
        ];
        let parameter_corners = [
            Point2::try_new(bounds[0][0], bounds[1][0])?,
            Point2::try_new(bounds[0][1], bounds[1][0])?,
            Point2::try_new(bounds[0][1], bounds[1][1])?,
            Point2::try_new(bounds[0][0], bounds[1][1])?,
        ];
        let trims = (0..4)
            .map(|side| {
                let trim_type = if singular[side] {
                    BrepTrimType::Singular
                } else if seam_sides[side] {
                    BrepTrimType::Seam
                } else {
                    BrepTrimType::Boundary
                };
                BrepTrim::try_new(
                    [corner_vertices[side], corner_vertices[(side + 1) % 4]],
                    edge_indices[side],
                    reversed_3d[side],
                    NurbsCurve2::try_line(
                        parameter_corners[side],
                        parameter_corners[(side + 1) % 4],
                    )?,
                    trim_type,
                    iso[side],
                    [0.0, 0.0],
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let face = BrepFace::try_new(
            surface,
            reversed,
            vec![BrepLoop::try_new(BrepLoopType::Outer, trims)?],
        )?;
        Self::try_new(vertices, edges, vec![face], tolerance)
    }
}

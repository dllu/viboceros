//! Exact underlying-surface restriction with unchanged visible B-rep topology.
use super::*;

#[path = "../../../../third_party/opennurbs_rust/brep_shrink.rs"]
mod interval_policy;

#[cfg(test)]
mod tests;

/// Margin policy for contracting underlying surfaces to their outer UV trims.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BrepSurfaceShrinkMode {
    /// Leave the OpenNURBS one-percent margin on non-isoparametric extrema.
    #[default]
    Standard,
    /// Crop to the outer trim bounds without that margin.
    ToEdge,
}

impl Brep {
    /// Restrict every underlying surface, retaining native UV coordinates,
    /// orientation, vertices, edges, loops, and numeric indices. Boundary
    /// curve weights are unchanged; the underlying control net changes by
    /// knot insertion and cropping.
    /// Knot insertion/cropping is exact NURBS algebra, without fitting.
    /// A face already at its outer bounds is unchanged. UV extrema use the
    /// kernel's tolerance-controlled rational bounds, including signed weights;
    /// unresolved poles or subdivision budgets fail before any output is returned.
    pub fn try_shrunk_surfaces(
        &self,
        mode: BrepSurfaceShrinkMode,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        self.shrink_surfaces(mode, tolerance, None)
    }

    /// Restrict only the indexed faces, preserving every unselected face and
    /// all shared vertices and spatial edges. Duplicate indices are accepted
    /// once. All indices are checked before cropping; an empty list is a no-op.
    pub fn try_shrunk_surface_faces(
        &self,
        faces: &[usize],
        mode: BrepSurfaceShrinkMode,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let mut selected = vec![false; self.faces.len()];
        for &face in faces {
            let entry = selected
                .get_mut(face)
                .ok_or(GeometryError::BrepFaceIndexOutOfRange {
                    face,
                    face_count: self.faces.len(),
                })?;
            *entry = true;
        }
        self.shrink_surfaces(mode, tolerance, Some(&selected))
    }

    fn shrink_surfaces(
        &self,
        mode: BrepSurfaceShrinkMode,
        tolerance: Tolerance,
        selected: Option<&[bool]>,
    ) -> Result<Self, GeometryError> {
        let mut result = self.clone();
        let mut changed = false;
        for (index, face) in result.faces.iter_mut().enumerate() {
            if selected.is_some_and(|selected| !selected[index]) {
                continue;
            }
            let outer = &face.loops[0];
            let mut bounds = None;
            let mut iso_ends = None;
            let mut all_iso = true;
            let mut surface_sides = [false; 4];
            for trim in &outer.trims {
                let [min, max] = trim_bounds(&trim.curve, &face.surface)?;
                include(&mut bounds, min);
                include(&mut bounds, max);
                match trim.iso {
                    SurfaceIso::NotIso => all_iso = false,
                    iso => {
                        if let Some(side) = match iso {
                            SurfaceIso::West => Some(0),
                            SurfaceIso::South => Some(1),
                            SurfaceIso::East => Some(2),
                            SurfaceIso::North => Some(3),
                            _ => None,
                        } {
                            surface_sides[side] = true;
                        }
                        include(&mut iso_ends, trim.curve.start_point()?);
                        include(&mut iso_ends, trim.curve.end_point()?);
                    }
                }
            }
            let domain = [face.surface.domain_u(), face.surface.domain_v()]
                .map(|range| [*range.start(), *range.end()]);
            let Some([u, v]) = interval_policy::intervals(
                domain,
                bounds.ok_or(GeometryError::EmptyPointSet)?,
                all_iso,
                iso_ends,
                surface_sides,
                mode == BrepSurfaceShrinkMode::Standard,
            )?
            else {
                continue;
            };
            face.surface = face.surface.try_trimmed(u[0]..=u[1], v[0]..=v[1])?;
            for trim in face
                .loops
                .iter_mut()
                .flat_map(|boundary| &mut boundary.trims)
            {
                if trim.iso != SurfaceIso::NotIso {
                    trim.iso = trim_iso::classify(&trim.curve, &face.surface);
                }
            }
            changed = true;
        }
        if changed {
            result.validate(tolerance)?;
        }
        Ok(result)
    }
}

fn include(bounds: &mut Option<[[Real; 2]; 2]>, point: Point2) {
    let point = [point.x(), point.y()];
    let bounds = bounds.get_or_insert(point.map(|p| [p, p]));
    for axis in 0..2 {
        bounds[axis][0] = bounds[axis][0].min(point[axis]);
        bounds[axis][1] = bounds[axis][1].max(point[axis]);
    }
}

/// Rhino's runtime loop boxes are tighter than the archive's control boxes
/// (observed with a rotated rational circle). Compute actual UV bounds in a
/// translated frame so the relative stopping rule cannot scale with a large
/// parameter origin. This is independent of the licensed interval policy.
fn trim_bounds(curve: &NurbsCurve2, surface: &NurbsSurface) -> Result<[Point2; 2], GeometryError> {
    // A same-sign rational linear span is exactly a line segment. Read its
    // active endpoints directly to preserve exact decimal isoparametric UVs
    // and avoid roundoff from translating an already exact bound.
    if curve.degree() == 1
        && curve.control_points().iter().all(|p| {
            p.weight().is_sign_positive() == curve.control_points()[0].weight().is_sign_positive()
        })
    {
        let mut bounds = None;
        for span in 1..curve.control_points().len() {
            if curve.knots()[span] < curve.knots()[span + 1] {
                include(&mut bounds, curve.control_points()[span - 1].point());
                include(&mut bounds, curve.control_points()[span].point());
            }
        }
        let [u, v] = bounds.ok_or(GeometryError::EmptyPointSet)?;
        return Ok([Point2::try_new(u[0], v[0])?, Point2::try_new(u[1], v[1])?]);
    }
    let domains = [surface.domain_u(), surface.domain_v()];
    let origin = domains.each_ref().map(|d| *d.start());
    let lengths = domains.each_ref().map(|d| *d.end() - *d.start());
    require_finite(lengths, "surface shrink parameter lengths")?;
    let absolute = (lengths[0].min(lengths[1]) * 1e-12).max(Real::from_bits(1));
    let tolerance =
        Tolerance::try_new(absolute, 16. * Real::EPSILON, Tolerance::DEFAULT.angular())?;
    let curve = NurbsCurve::try_new_rational(
        curve.degree(),
        curve
            .control_points()
            .iter()
            .map(|p| {
                WeightedPoint3::try_new(
                    Point3::try_new(p.point().x() - origin[0], p.point().y() - origin[1], 0.)?,
                    p.weight(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
        curve.knots().to_vec(),
    )?;
    let bounds = curve.tight_bounds(tolerance)?;
    let point = |p: Point3| Point2::try_new(p.x() + origin[0], p.y() + origin[1]);
    Ok([point(bounds.min())?, point(bounds.max())?])
}

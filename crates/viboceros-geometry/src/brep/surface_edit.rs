//! Rebuild model-space boundaries after an underlying control-net edit.
use super::*;
use crate::PointMorph;

const MAX_TRIM_IMAGE_CONTROLS: usize = 4096;

struct SurfaceImage<'a>(&'a NurbsSurface);
impl PointMorph for SurfaceImage<'_> {
    fn morph_point(&self, p: Point3) -> Result<Point3, GeometryError> {
        self.0.evaluate(p.x(), p.y())
    }
}

impl Brep {
    /// Replace a single face's underlying surface, retaining UV trims and face
    /// orientation. Rectangular boundaries use exact isocurves. Other trim
    /// images use the bounded native-parameter curve fitter; its accuracy checks
    /// are sampled. The assembled result must pass ordinary B-rep validation.
    /// No component tolerance is enlarged to accept an inconsistent boundary.
    pub fn try_with_edited_single_surface(
        &self,
        surface: NurbsSurface,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let [face] = self.faces.as_slice() else {
            return invalid("an underlying surface edit requires one face");
        };
        if surface.domain_u() != face.surface.domain_u()
            || surface.domain_v() != face.surface.domain_v()
        {
            return invalid("an underlying surface edit must retain its UV domains");
        }
        if surface == face.surface {
            return Ok(self.clone());
        }
        if let Some(bounds) = face.rectangular_trim_bounds(tolerance)? {
            return Self::try_rectangular_surface_face_with_orientation(
                surface,
                bounds[0][0]..=bounds[0][1],
                bounds[1][0]..=bounds[1][1],
                face.reversed,
                tolerance,
            );
        }
        let fitting = Tolerance::try_new(
            (tolerance.absolute() * 0.25).max(Real::MIN_POSITIVE),
            tolerance.relative(),
            tolerance.angular(),
        )?;
        let mut vertices = Vec::<BrepVertex>::new();
        let mut vertex_sources = Vec::<usize>::new();
        let mut edges = Vec::<BrepEdge>::new();
        let mut edge_sources = BTreeMap::new();
        let mut loops = face.loops.clone();
        for trim in loops.iter_mut().flat_map(|l| &mut l.trims) {
            let old_vertices = trim.vertices;
            let mut mapped = [0; 2];
            for (end, uv) in [trim.curve.start_point()?, trim.curve.end_point()?]
                .into_iter()
                .enumerate()
            {
                let p = surface.evaluate(uv.x(), uv.y())?;
                mapped[end] = match vertices.iter().enumerate().find(|(i, v)| {
                    vertex_sources[*i] == old_vertices[end]
                        && v.point
                            .distance_to(p)
                            .is_ok_and(|d| d <= tolerance.absolute())
                }) {
                    Some((i, _)) => i,
                    None => {
                        let i = vertices.len();
                        vertices.push(BrepVertex::try_new(p, tolerance.absolute())?);
                        vertex_sources.push(old_vertices[end]);
                        i
                    }
                };
            }
            let uv = trim_image::LiftedTrim::new(trim, &surface)?.curve;
            let mut spatial = crate::morph::fit_curve_with_control_limit(
                &SurfaceImage(&surface),
                &uv,
                fitting,
                MAX_TRIM_IMAGE_CONTROLS,
            )?;
            trim.vertices = mapped;
            if spatial
                .control_points()
                .iter()
                .all(|p| p.point() == spatial.control_points()[0].point())
            {
                if mapped[0] != mapped[1] {
                    return invalid("a collapsed edited trim has distinct endpoint vertices");
                }
                trim.edge = None;
                trim.reversed_3d = false;
                trim.trim_type = BrepTrimType::Singular;
                continue;
            }
            let reversed = trim.reversed_3d;
            if reversed {
                spatial = spatial.reversed()?;
            }
            let edge_vertices = if reversed {
                [mapped[1], mapped[0]]
            } else {
                mapped
            };
            let key = (trim.edge, edge_vertices);
            let index = if let Some(&i) = edge_sources.get(&key) {
                i
            } else {
                let i = edges.len();
                edges.push(BrepEdge::try_new(
                    edge_vertices,
                    spatial,
                    tolerance.absolute(),
                )?);
                // Missing source edges were singular, and must not accidentally
                // share a newly nonconstant boundary with another singular trim.
                if trim.edge.is_some() {
                    edge_sources.insert(key, i);
                }
                i
            };
            trim.edge = Some(index);
            trim.trim_type = BrepTrimType::Boundary;
        }
        let mut uses = vec![Vec::new(); edges.len()];
        for (loop_index, face_loop) in loops.iter().enumerate() {
            for trim in &face_loop.trims {
                if let Some(edge) = trim.edge {
                    uses[edge].push(loop_index);
                }
            }
        }
        for (loop_index, face_loop) in loops.iter_mut().enumerate() {
            for trim in &mut face_loop.trims {
                if let Some(edge) = trim.edge {
                    trim.trim_type = if uses[edge].len() == 1 {
                        BrepTrimType::Boundary
                    } else if uses[edge].iter().filter(|&&i| i == loop_index).count() >= 2 {
                        BrepTrimType::Seam
                    } else {
                        BrepTrimType::Mated
                    };
                }
            }
        }
        Self::try_new(
            vertices,
            edges,
            vec![BrepFace::try_new(surface, face.reversed, loops)?],
            tolerance,
        )
    }
}

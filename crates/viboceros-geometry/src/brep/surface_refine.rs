//! Shape-preserving surface refinement retains already-validated trim topology.
use super::*;
impl Brep {
    /// Insert an existing surface knot operation into one face's underlying
    /// surface. Exact refinement preserves its domain and parameterized locus,
    /// so all model-space edges, vertices and UV trims remain valid. Avoid
    /// re-fitting or re-certifying those unchanged boundary images.
    pub fn try_insert_single_surface_knot(
        &self,
        direction: crate::SurfaceKnotDirection,
        parameter: Real,
        multiplicity: usize,
    ) -> Result<Self, GeometryError> {
        let [face] = self.faces.as_slice() else {
            return invalid("surface knot insertion requires one face");
        };
        let surface = match direction {
            crate::SurfaceKnotDirection::U => {
                face.surface.try_insert_knot_u(parameter, multiplicity)?
            }
            crate::SurfaceKnotDirection::V => {
                face.surface.try_insert_knot_v(parameter, multiplicity)?
            }
            crate::SurfaceKnotDirection::Both => {
                return invalid("surface knot insertion requires a single direction");
            }
        };
        let mut result = self.clone();
        result.faces[0].surface = surface;
        Ok(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::SurfaceKnotDirection::{U, V};
    #[test]
    fn surface_refinement_keeps_topology_and_exact_parameterized_locus() {
        let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
        let surface = NurbsSurface::try_bilinear([
            p(0., 0., 0.),
            p(8., 0., 0.),
            p(8., 6., 2.),
            p(0., 6., 0.),
        ])
        .unwrap();
        let source = Brep::try_surface_face(surface.clone(), Tolerance::DEFAULT).unwrap();
        let result = source
            .try_insert_single_surface_knot(U, 0.35, 1)
            .unwrap()
            .try_insert_single_surface_knot(V, 0.6, 1)
            .unwrap();
        assert_eq!(result.vertices, source.vertices);
        assert_eq!(result.edges, source.edges);
        assert_eq!(result.faces[0].loops, source.faces[0].loops);
        assert_eq!(result.faces[0].reversed, source.faces[0].reversed);
        assert_eq!(
            result.faces[0].surface,
            surface
                .try_insert_knot_u(0.35, 1)
                .unwrap()
                .try_insert_knot_v(0.6, 1)
                .unwrap()
        );
        for u in 0..11 {
            for v in 0..11 {
                assert!(
                    surface
                        .evaluate(u as Real / 10., v as Real / 10.)
                        .unwrap()
                        .distance_to(
                            result.faces[0]
                                .surface
                                .evaluate(u as Real / 10., v as Real / 10.)
                                .unwrap()
                        )
                        .unwrap()
                        < 1e-12
                );
            }
        }
        assert_eq!(
            result.try_insert_single_surface_knot(U, 0.35, 1).unwrap(),
            result
        );
        assert!(source.try_insert_single_surface_knot(U, 2., 1).is_err());
        assert!(source.try_insert_single_surface_knot(U, 0.35, 9).is_err());
        let solid = Brep::try_box(
            Frame3::try_from_normal(
                p(0., 0., 0.),
                Vector3::try_new(0., 0., 1.).unwrap(),
                Tolerance::DEFAULT,
            )
            .unwrap(),
            [[0., 1.], [0., 1.], [0., 1.]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(solid.try_insert_single_surface_knot(U, 0.35, 1).is_err());
    }
}

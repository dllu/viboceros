//! Prepared control cages for display deformation, without changing model topology.
use super::*;
use crate::{CurvePreviewCage, PointMorph, SurfacePreviewCage};

struct IsoWire {
    varying: usize,
    fixed: Real,
    intervals: Vec<[Real; 2]>,
}

struct FaceCage {
    surface: SurfacePreviewCage,
    wires: Vec<IsoWire>,
}

/// Prepared control cages and low-degree polynomial interpolation grids for display.
/// Trim intervals and degree elevation are prepared once. Changing a morph
/// maps controls or cached samples and extracts isocurves. The wires are not an assembled
/// or tolerance-certified B-rep.
pub struct BrepWireCage {
    edges: Vec<CurvePreviewCage>,
    faces: Vec<FaceCage>,
}

impl BrepWireCage {
    pub fn try_new(
        brep: &Brep,
        wire_density: i32,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if !(crate::MIN_SURFACE_WIRE_DENSITY..=crate::MAX_SURFACE_WIRE_DENSITY)
            .contains(&wire_density)
        {
            return Err(GeometryError::InvalidSurfaceWireDensity(wire_density));
        }
        if brep.edges.len() > crate::MAX_SURFACE_WIRES {
            return Err(GeometryError::TooManySurfaceWires);
        }
        let edges = brep
            .edges
            .iter()
            .map(|e| CurvePreviewCage::try_new(&e.curve))
            .collect::<Result<Vec<_>, _>>()?;
        if wire_density < 0 {
            return Ok(Self {
                edges,
                faces: Vec::new(),
            });
        }
        let mut faces = Vec::with_capacity(brep.faces.len());
        let mut wire_count = edges.len();
        for face in &brep.faces {
            let frame = face.local_parameter_frame()?;
            let surface = SurfacePreviewCage::try_new(&frame.face.surface)?;
            let mut wires = Vec::new();
            for (varying, parameters) in [
                (0, frame.face.surface.wire_parameters_v(wire_density)?),
                (1, frame.face.surface.wire_parameters_u(wire_density)?),
            ] {
                let interior = parameters.len().saturating_sub(2);
                for fixed in parameters.into_iter().skip(1).take(interior) {
                    let intervals =
                        trimmed_isocurve_intervals(&frame.face, varying, fixed, tolerance)?;
                    wire_count = wire_count
                        .checked_add(intervals.len())
                        .ok_or(GeometryError::TooManySurfaceWires)?;
                    if wire_count > crate::MAX_SURFACE_WIRES {
                        return Err(GeometryError::TooManySurfaceWires);
                    }
                    wires.push(IsoWire {
                        varying,
                        fixed,
                        intervals,
                    });
                }
            }
            faces.push(FaceCage { surface, wires });
        }
        Ok(Self { edges, faces })
    }

    pub fn morphed_wires(
        &self,
        morph: &(impl PointMorph + ?Sized),
        preserve_structure: bool,
    ) -> Result<Vec<NurbsCurve>, GeometryError> {
        let mut wires = self
            .edges
            .iter()
            .map(|c| c.morphed_curve(morph, preserve_structure))
            .collect::<Result<Vec<_>, _>>()?;
        for face in &self.faces {
            let surface = face.surface.morphed_surface(morph, preserve_structure)?;
            for iso in &face.wires {
                let curve = if iso.varying == 0 {
                    surface.isocurve_u(iso.fixed)?
                } else {
                    surface.isocurve_v(iso.fixed)?
                };
                for curve in trim_isocurve_to_intervals(curve, iso.intervals.clone())? {
                    push_brep_wire(&mut wires, curve)?;
                }
            }
        }
        Ok(wires)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Circle3, TwistPointMorph};

    #[test]
    fn prepared_rational_trim_holes_and_large_uv_origins_survive_cage_motion() {
        let tolerance = Tolerance::DEFAULT;
        let origin = Point3::try_new(0., 0., 5.).unwrap();
        let normal = UnitVector3::try_new(0., 0., 1., tolerance).unwrap();
        let outer = Circle3::try_new(origin, 5., normal, tolerance)
            .unwrap()
            .to_nurbs()
            .unwrap();
        let hole = Circle3::try_new(origin, 2., normal, tolerance)
            .unwrap()
            .to_nurbs()
            .unwrap();
        let ring = Brep::try_planar_face_with_holes(&outer, &[hole], tolerance).unwrap();
        let saved = ring.clone();
        for offset in [[0., 0.], [1e12, -2e12]] {
            let mut shifted = ring.clone();
            shifted.faces[0] =
                super::super::parameter_frame::tests::translated_face(&ring.faces[0], offset);
            let expected = ring.wireframe_curves(1, tolerance).unwrap();
            let cage = BrepWireCage::try_new(&shifted, 1, tolerance).unwrap();
            let cage_edges = BrepWireCage::try_new(&shifted, -1, tolerance).unwrap();
            for angle in [0_f64, 90., -450.] {
                let morph = TwistPointMorph::try_new(
                    Point3::try_new(0., 0., 0.).unwrap(),
                    Point3::try_new(0., 0., 10.).unwrap(),
                    angle.to_radians(),
                    false,
                    tolerance,
                )
                .unwrap();
                let wires = cage.morphed_wires(&morph, false).unwrap();
                assert_eq!(wires.len(), 6);
                assert_eq!(cage_edges.morphed_wires(&morph, false).unwrap().len(), 2);
                for (a, b) in wires.iter().zip(&expected) {
                    let da = a.domain();
                    let db = b.domain();
                    for i in 0..=16 {
                        let t = i as Real / 16.;
                        let actual = a
                            .evaluate(da.start() + (da.end() - da.start()) * t)
                            .unwrap();
                        let expected = morph
                            .morph_point(
                                b.evaluate(db.start() + (db.end() - db.start()) * t)
                                    .unwrap(),
                            )
                            .unwrap();
                        assert!(
                            actual.distance_to(expected).unwrap() < 1e-8,
                            "angle {angle}, UV {offset:?}"
                        );
                    }
                }
                // The four interior segments stop at radius two instead of crossing the hole.
                for wire in wires.iter().skip(2) {
                    let d = wire.domain();
                    for i in 0..=16 {
                        let p = wire
                            .evaluate(d.start() + (d.end() - d.start()) * i as Real / 16.)
                            .unwrap();
                        assert!(p.x().hypot(p.y()) >= 2. - 1e-8);
                    }
                }
            }
        }
        assert_eq!(ring, saved);
    }
}

//! Truncated cone cages reuse mesh sampling and the public cylinder builder.
use super::*;
use crate::{Frame3, MeshCapFaceStyle, MeshTruncatedConeOptions, SubDCylinderCap, Tolerance};
impl SubD {
    /// Smooth capped frusta use control-net radii. Open frusta use surface
    /// radii, retaining the public cylinder's boundary tags and radial scale.
    /// Either end radius may be zero, producing the corresponding rounded cone.
    pub fn try_truncated_cone(
        frame: Frame3,
        radii: [f64; 2],
        height: f64,
        counts: [usize; 2],
        cap: SubDCylinderCap,
    ) -> Result<Self, GeometryError> {
        if radii.iter().chain([&height]).any(|v| !v.is_finite()) {
            return Err(GeometryError::NonFinite {
                context: "SubD truncated-cone dimensions",
            });
        }
        let end = frame.point_at([0., 0., height])?;
        if height <= 0.
            || radii.iter().any(|&r| r < 0.)
            || radii == [0.; 2]
            || end == frame.origin()
            || radii[0] > 0. && frame.point_at([radii[0], 0., 0.])? == frame.origin()
            || radii[1] > 0. && frame.with_origin(end).point_at([radii[1], 0., 0.])? == end
        {
            return Err(GeometryError::Degenerate {
                context: "SubD truncated-cone dimensions at model origin",
            });
        }
        let closed = cap != SubDCylinderCap::None;
        if radii.contains(&0.) {
            let base = if radii[0] == 0. {
                Frame3::try_from_directions(
                    end,
                    frame.x_axis().as_vector(),
                    frame.y_axis().as_vector().scaled(-1.)?,
                    Tolerance::NUMERICAL_VALIDATION,
                )?
            } else {
                frame
            };
            // A zero base from FitPoints has no valid cap construction; native
            // retains the open reverse-cone cage even with Solid=Yes.
            let cone_cap = if radii[0] == 0. {
                SubDCylinderCap::None
            } else {
                cap
            };
            let radius = radii[0].max(radii[1]);
            if closed {
                return Self::try_cone(base, radius, height, counts, cone_cap);
            }
            // Solve on a unit cage, so numeric admission is independent of
            // model translation and scale. Limit masks commute with affine maps.
            let cone = Self::try_cone(unit_frame()?, 1., 1., counts, cone_cap)?;
            let fit = cone.try_with_limit_points(
                cone.control_points(),
                Tolerance::try_new(128. * f64::EPSILON, 1e-12, 1e-10)?,
            )?;
            let points = fit
                .control_points()
                .iter()
                .map(|p| {
                    let [x, y, z] = p.to_array();
                    base.point_at([radius * x, radius * y, height * z])
                })
                .collect::<Result<Vec<_>, _>>()?;
            return fit.try_with_control_points(&points);
        }
        let [around, vertical] = counts;
        let simple = around == 3 || around == 4 && cap == SubDCylinderCap::Quads;
        let one_cap = if !closed {
            0
        } else if simple {
            1
        } else if cap == SubDCylinderCap::Quads && around.is_multiple_of(2) {
            around / 2
        } else {
            around
        };
        let vertices = vertical
            .checked_add(1)
            .and_then(|v| v.checked_mul(around))
            .and_then(|v| v.checked_add(2 * usize::from(closed && !simple)));
        let faces = around
            .checked_mul(vertical)
            .and_then(|v| one_cap.checked_mul(2).and_then(|c| v.checked_add(c)));
        if around < 3
            || vertical < 2
            || vertices.is_none_or(|v| v > 100000)
            || faces.is_none_or(|v| v > 100000)
        {
            return Err(viboceros_opennurbs::SubDError(
                "Invalid or excessive SubD truncated-cone topology".into(),
            )
            .into());
        }
        if !closed {
            let unit = Self::try_cylinder(unit_frame()?, 1., 1., counts, SubDCylinderCap::None)?;
            let points = unit
                .control_points()
                .iter()
                .map(|p| {
                    let [x, y, z] = p.to_array();
                    let r = z.mul_add(radii[1] - radii[0], radii[0]);
                    frame.point_at([x * r, y * r, z * height])
                })
                .collect::<Result<Vec<_>, _>>()?;
            return unit.try_with_control_points(&points);
        }
        let mesh = TriangleMesh::try_truncated_cone_grid(
            frame,
            radii,
            height,
            MeshTruncatedConeOptions {
                vertical_count: vertical,
                around_count: around,
                solid: true,
                cap_style: if cap == SubDCylinderCap::Quads {
                    MeshCapFaceStyle::Quadrilaterals
                } else {
                    MeshCapFaceStyle::Triangles
                },
            },
            Tolerance::NUMERICAL_VALIDATION,
        )?;
        if around == 3 {
            let wall = around * (vertical + 1);
            let points = mesh.vertices()[..wall].to_vec();
            let mut faces = mesh.faces()[..around * vertical]
                .iter()
                .map(|f| f.indices().to_vec())
                .collect::<Vec<_>>();
            faces.push((0..around as u32).rev().collect());
            faces.push(((wall - around) as u32..wall as u32).collect());
            return Self::try_from_polygons(points, faces);
        }
        let mesh = mesh.combined_identical_vertices().0;
        Self::try_from_polygons(
            mesh.vertices().to_vec(),
            mesh.faces().iter().map(|f| f.indices().to_vec()).collect(),
        )
    }
}
fn unit_frame() -> Result<Frame3, GeometryError> {
    Frame3::try_from_directions(
        Point3::try_new(0., 0., 0.)?,
        Vector3::try_new(1., 0., 0.)?,
        Vector3::try_new(0., 1., 0.)?,
        Tolerance::NUMERICAL_VALIDATION,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn open_zero_end_interpolates_the_ring_stations_and_apex_across_model_scales() {
        for scale in [1e-6, 1., 1e6] {
            let frame = Frame3::try_from_normal(
                Point3::try_new(10. * scale, 20. * scale, 30. * scale).unwrap(),
                Vector3::try_new(1., 2., 3.).unwrap(),
                Tolerance::NUMERICAL_VALIDATION,
            )
            .unwrap();
            let radius = 3. * scale;
            let height = 5. * scale;
            let subd = SubD::try_truncated_cone(
                frame,
                [radius, 0.],
                height,
                [8, 2],
                SubDCylinderCap::None,
            )
            .unwrap();
            assert_eq!(subd.control_points().len(), 17);
            for point in subd.surface_points() {
                let [x, y, z] = frame.coordinates_of(*point).unwrap();
                let fraction = z / height;
                assert!([0., 0.5, 1.].iter().any(|t| (fraction - t).abs() < 1e-12));
                assert!((x.hypot(y) - radius * (1. - fraction)).abs() < scale * 1e-11);
            }
        }
    }
    #[test]
    fn truncated_cone_rejects_invalid_dimensions_and_unbounded_counts() {
        let f = Frame3::try_from_normal(
            Point3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_new(0., 0., 1.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        for (r, h) in [
            ([0., 0.], 5.),
            ([-1., 1.], 5.),
            ([3., -1.], 5.),
            ([f64::NAN, 1.], 5.),
            ([3., 1.], 0.),
            ([3., 1.], f64::INFINITY),
        ] {
            assert!(SubD::try_truncated_cone(f, r, h, [8, 2], SubDCylinderCap::Quads).is_err());
        }
        for counts in [[2, 2], [8, 1], [usize::MAX, usize::MAX], [100000, 100000]] {
            assert!(
                SubD::try_truncated_cone(f, [3., 1.], 5., counts, SubDCylinderCap::None).is_err()
            );
        }
        assert!(
            SubD::try_truncated_cone(
                f.with_origin(Point3::try_new(1e30, 1e30, 1e30).unwrap()),
                [3., 1.],
                5.,
                [8, 2],
                SubDCylinderCap::Quads
            )
            .is_err()
        );
    }
}

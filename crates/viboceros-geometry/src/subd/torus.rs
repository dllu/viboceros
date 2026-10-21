//! Periodic torus nets reuse mesh sampling and native circular limit placement.
use super::*;
use crate::{Frame3, MeshTorusOptions, SubDCylinderCap, Tolerance};
impl SubD {
    /// Construct a periodic quad torus with the requested vertex limit stations.
    /// The circle limit masks separate in the two regular grid directions.
    pub fn try_torus(
        frame: Frame3,
        major_radius: f64,
        minor_radius: f64,
        counts: [usize; 2],
    ) -> Result<Self, GeometryError> {
        let [around, vertical] = counts;
        if around < 3 || vertical < 3 || around.checked_mul(vertical).is_none_or(|n| n > 100_000) {
            return Err(viboceros_opennurbs::SubDError(
                "Invalid or excessive SubD torus topology".into(),
            )
            .into());
        }
        if !major_radius.is_finite() || !minor_radius.is_finite() {
            return Err(GeometryError::NonFinite {
                context: "SubD torus radii",
            });
        }
        if major_radius <= 0.
            || minor_radius <= 0.
            || frame.point_at([major_radius, 0., 0.])? == frame.origin()
            || frame.point_at([0., 0., minor_radius])? == frame.origin()
        {
            return Err(GeometryError::Degenerate {
                context: "SubD torus dimensions at model origin",
            });
        }
        let unit = Frame3::try_from_directions(
            Point3::try_new(0., 0., 0.)?,
            Vector3::try_new(1., 0., 0.)?,
            Vector3::try_new(0., 1., 0.)?,
            Tolerance::NUMERICAL_VALIDATION,
        )?;
        let circle_scale = |n| -> Result<f64, GeometryError> {
            let ring = Self::try_cylinder(unit, 1., 1., [n, 1], SubDCylinderCap::None)?;
            let p = ring.control_points()[0];
            Ok(p.x().hypot(p.y()))
        };
        let u = circle_scale(around)?;
        let v = if vertical == around {
            u
        } else {
            circle_scale(vertical)?
        };
        let scaffold = TriangleMesh::try_torus_grid(
            unit,
            2.,
            1.,
            MeshTorusOptions {
                around_count: around,
                vertical_count: vertical,
            },
            Tolerance::NUMERICAL_VALIDATION,
        )?;
        let points = scaffold
            .vertices()
            .iter()
            .map(|p| {
                let [x, y, z] = p.to_array();
                let radial = x.hypot(y);
                let control_radius = u * minor_radius.mul_add(v * (radial - 2.), major_radius);
                frame.point_at([
                    control_radius * (x / radial),
                    control_radius * (y / radial),
                    minor_radius * v * z,
                ])
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::try_from_polygons(
            points,
            scaffold
                .faces()
                .iter()
                .map(|f| f.indices().to_vec())
                .collect(),
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn frame() -> Frame3 {
        Frame3::try_from_normal(
            Point3::try_new(10., 20., 30.).unwrap(),
            Vector3::try_new(1., 2., 3.).unwrap(),
            Tolerance::NUMERICAL_VALIDATION,
        )
        .unwrap()
    }
    #[test]
    fn torus_limit_stations_match_the_shared_grid_and_remain_periodic() {
        for counts in [[3, 3], [4, 4], [7, 5], [8, 6], [12, 9]] {
            let subd = SubD::try_torus(frame(), 5., 2., counts).unwrap();
            let grid = TriangleMesh::try_torus_grid(
                frame(),
                5.,
                2.,
                MeshTorusOptions {
                    around_count: counts[0],
                    vertical_count: counts[1],
                },
                Tolerance::NUMERICAL_VALIDATION,
            )
            .unwrap();
            assert_eq!(subd.control_points().len(), counts[0] * counts[1]);
            assert_eq!(subd.faces().len(), counts[0] * counts[1]);
            assert_eq!(subd.edges().len(), 2 * counts[0] * counts[1]);
            assert!(subd.vertex_tags().iter().all(|&tag| tag == 1));
            assert!(subd.edge_tags().iter().all(|&tag| tag == 1));
            for (actual, target) in subd.surface_points().iter().zip(grid.vertices()) {
                assert!(actual.distance_to(*target).unwrap() < 1e-11);
            }
        }
    }
    #[test]
    fn torus_rejects_invalid_dimensions_and_topology_before_allocation() {
        for counts in [[2, 3], [3, 2], [usize::MAX, 3], [100000, 100000]] {
            assert!(SubD::try_torus(frame(), 5., 2., counts).is_err());
        }
        for (major, minor) in [
            (0., 1.),
            (1., 0.),
            (-1., 1.),
            (1., -1.),
            (f64::NAN, 1.),
            (1., f64::INFINITY),
        ] {
            assert!(SubD::try_torus(frame(), major, minor, [8, 6]).is_err());
        }
        assert!(
            SubD::try_torus(
                frame().with_origin(Point3::try_new(1e30, 1e30, 1e30).unwrap()),
                5.,
                2.,
                [8, 6]
            )
            .is_err()
        );
    }
}

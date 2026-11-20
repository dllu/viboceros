//! Return section and seam extensions for a profiled polygonal loop.
use super::*;
use crate::SurfaceExtensionEdge;

pub(super) struct LoopSeam {
    pub last: NurbsCurve,
    tangent_angle: Real,
}

impl LoopSeam {
    pub(super) fn try_new(
        rail: &NurbsCurve,
        first: &NurbsCurve,
        style: SweepFrameStyle,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let domain = rail.domain();
        let parameters = [*domain.start(), *domain.end()];
        let rail = CurveRef::NurbsCurve(rail);
        let frames = match style {
            SweepFrameStyle::Freeform => rail.rotation_minimizing_frames_from_side(
                &parameters,
                None,
                ParameterSide::Right,
                FrameTransportOptions {
                    side: ParameterSide::Left,
                    ..Default::default()
                },
            )?,
            SweepFrameStyle::Roadlike(axis) => axis_frames(rail, &parameters, |_| Ok(axis))?,
        };
        let first_frame = frames[0];
        let last_frame = frames[1];
        let tangent_angle = first_frame
            .z_axis()
            .as_vector()
            .angle_to(last_frame.z_axis().as_vector())?;
        let same_frame = first_frame.origin() == last_frame.origin()
            && first_frame
                .x_axis()
                .as_vector()
                .angle_to(last_frame.x_axis().as_vector())?
                <= tolerance.angular()
            && tangent_angle <= tolerance.angular();
        let last = if same_frame {
            first.clone()
        } else {
            map_profile(first, |point| {
                last_frame.point_at(first_frame.coordinates_of(point)?)
            })?
        };
        Ok(Self {
            last,
            tangent_angle,
        })
    }

    pub(super) fn extensions(
        &self,
        first: &NurbsSurface,
        last: &NurbsSurface,
        tolerance: Tolerance,
    ) -> Result<[NurbsSurface; 2], GeometryError> {
        if self.tangent_angle <= tolerance.angular() {
            return Err(invalid(
                "a smooth polygonal sweep seam requires UntrimmedMiters=Yes",
            ));
        }
        let start = *first.domain_u().start();
        let finish = *last.domain_u().end();
        let distance = extension_distance(&first.isocurve_v(start)?, &last.isocurve_v(finish)?)?;
        let west = first.try_extended_linearly_by_length(SurfaceExtensionEdge::West, distance)?;
        let east = last.try_extended_linearly_by_length(SurfaceExtensionEdge::East, distance)?;
        Ok([
            west.try_trimmed_u(*west.domain_u().start()..=start)?,
            east.try_trimmed_u(finish..=*east.domain_u().end())?,
        ])
    }
}

fn map_profile(
    curve: &NurbsCurve,
    mut map: impl FnMut(Point3) -> Result<Point3, GeometryError>,
) -> Result<NurbsCurve, GeometryError> {
    let controls = curve
        .control_points()
        .iter()
        .map(|c| WeightedPoint3::try_new(map(c.point())?, c.weight()))
        .collect::<Result<Vec<_>, _>>()?;
    NurbsCurve::try_new_rational(curve.degree(), controls, curve.knots().to_vec())
}

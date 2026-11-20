//! Return section and seam extensions for a profiled polygonal loop.
use super::*;
use crate::sweep::miter::surface_between;

pub(super) struct LoopSeam {
    pub last: NurbsCurve,
    first_frame: Frame3,
    last_frame: Frame3,
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
        let same_frame = first_frame.origin() == last_frame.origin()
            && first_frame
                .x_axis()
                .as_vector()
                .angle_to(last_frame.x_axis().as_vector())?
                <= tolerance.angular()
            && first_frame
                .z_axis()
                .as_vector()
                .angle_to(last_frame.z_axis().as_vector())?
                <= tolerance.angular();
        let last = if same_frame {
            first.clone()
        } else {
            map_profile(first, |point| {
                last_frame.point_at(first_frame.coordinates_of(point)?)
            })?
        };
        Ok(Self {
            last,
            first_frame,
            last_frame,
        })
    }

    pub(super) fn extensions(
        self,
        first: &NurbsCurve,
        finish: Real,
        tolerance: Tolerance,
    ) -> Result<[NurbsSurface; 2], GeometryError> {
        let incoming = self.last_frame.z_axis().as_vector();
        let outgoing = self.first_frame.z_axis().as_vector();
        if incoming.angle_to(outgoing)? <= tolerance.angular() {
            return Err(invalid(
                "a smooth polygonal sweep seam requires UntrimmedMiters=Yes",
            ));
        }
        if first.is_closed()? {
            return Err(invalid(
                "a closed polygonal seam profile requires UntrimmedMiters=Yes",
            ));
        }
        let bounds = first
            .control_point_bounds()
            .union(self.last.control_point_bounds())?;
        let distance = bounds.min().distance_to(bounds.max())?;
        let back = outgoing.scaled(-distance)?;
        let ahead = incoming.scaled(distance)?;
        let start = map_profile(first, |p| p.translated(back))?;
        let end = map_profile(&self.last, |p| p.translated(ahead))?;
        Ok([
            surface_between(&[start, first.clone()], [-distance, 0.])?,
            surface_between(&[self.last, end], [finish, finish + distance])?,
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

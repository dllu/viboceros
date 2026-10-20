//! Face-constrained point input shares the signed-distance offset producer.
use super::*;
impl Plan {
    pub fn prepare_through_point(
        doc: &Document,
        source: Source,
        target: Target,
        point: Point3,
        options: Options,
    ) -> Result<Self, CommandError> {
        options.validate()?;
        if !source.current(doc) || !target.current(doc) {
            return Err(CommandError::Usage(
                "Select the source curve and surface again",
            ));
        }
        let constrained = target.location(point, false)?.point;
        let inverse_source = if target.surface.plane(doc.tolerance())?.is_some() {
            let boundaries = Brep::try_surface_face((*target.surface).clone(), doc.tolerance())?
                .edges()
                .iter()
                .map(|e| e.curve().clone())
                .collect::<Vec<_>>();
            extend_planar_offset(source.curve().clone(), &boundaries, doc.tolerance())?
        } else {
            source.curve().clone()
        };
        let inverse = viboceros_geometry::surface_curve_offset_distance_through_point(
            &target.surface,
            &inverse_source,
            constrained,
            doc.tolerance(),
        )?;
        if inverse.signed_distance.abs() <= doc.tolerance().absolute() {
            return Err(GeometryError::OffsetThroughPointNoSolution.into());
        }
        let signed = inverse.signed_distance * if target.reversed { -1. } else { 1. };
        // ThroughPoint ignores Number, the initial picked side and Flip. Each
        // accepted point addresses the original source, never a prior copy.
        let mut plan = Plan::prepare(
            doc,
            source,
            target,
            None,
            Options {
                distance: signed,
                number: 1,
                flip: false,
            },
        )?;
        let mut minimum = Real::INFINITY;
        for curve in &plan.curves {
            let parameter = curve.closest_parameter(constrained, doc.tolerance())?;
            minimum = minimum.min(curve.evaluate(parameter)?.distance_to(constrained)?);
        }
        if minimum > doc.tolerance().absolute() {
            return Err(GeometryError::OffsetThroughPointNoSolution.into());
        }
        plan.options = options;
        plan.through = Some(point);
        Ok(plan)
    }
}

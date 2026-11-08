//! Preserve the qualified Chamfer style policy around shared continuation geometry.
use super::*;
use crate::curve_pair_edit::boundary_continuation::{self, Styles};
pub(super) fn to_boundary(
    source: &Curve3,
    boundary: &Curve3,
    pick: Point3,
    options: Preferences,
    tolerance: Tolerance,
) -> Result<Curve3, CommandError> {
    // Rhino's mixed-style nonmeeting arc/closed-curve branch is not yet
    // qualified: rigidly transformed inputs selected different continuations.
    // Reject before producing a plan rather than admitting divergent geometry.
    if matches!(source, Curve3::Arc(_))
        && options.styles.arc == CurveArcExtensionStyle::Line
        && options.styles.other == CurveOtherExtensionStyle::Smooth
    {
        return Err(CommandError::Usage(
            "Nonmeeting arc chamfers against closed curves require ExtendOtherCurvesBy=Line",
        ));
    }
    boundary_continuation::to_boundary(
        source,
        boundary,
        pick,
        Styles {
            arc: options.styles.arc,
            other: options.styles.other,
        },
        tolerance,
    )
}

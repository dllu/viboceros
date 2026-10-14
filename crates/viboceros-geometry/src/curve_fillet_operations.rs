//! Exact edit and contact producers within shared curve-filleting policy.
use crate::{CircularArc3, CurveExtensionOperations, CurveSegment3, NurbsCurve, Real, Tolerance};
use std::ops::RangeInclusive;

/// Contact parameters refer to the original oriented leaves: first end,
/// second start. The arc follows that same direction.
pub struct CurveFilletContact {
    pub parameters: [Real; 2],
    pub arc: CircularArc3,
}
pub type CurveFilletContactProducer<'a, E> = dyn FnMut(&CurveSegment3, &CurveSegment3, Real, Tolerance) -> Result<Option<CurveFilletContact>, E>
    + 'a;
type NurbsTrim<'a, E> = dyn FnMut(&NurbsCurve, RangeInclusive<Real>) -> Result<NurbsCurve, E> + 'a;

/// `None` from the contact producer requests the shared solver for input bases
/// the backend cannot represent. Producer failures remain errors.
pub struct CurveFilletOperations<'a, E> {
    pub trim: &'a mut NurbsTrim<'a, E>,
    pub contacts: &'a mut CurveFilletContactProducer<'a, E>,
}

/// Nonmeeting-end preparation and corner contacts use their respective producers.
pub struct CurveFilletPairOperations<'a, E> {
    pub extensions: CurveExtensionOperations<'a, E>,
    pub contacts: &'a mut CurveFilletContactProducer<'a, E>,
}
impl<'a, E> CurveFilletPairOperations<'a, E> {
    pub fn new(
        extensions: &'a mut CurveExtensionOperations<'_, E>,
        contacts: &'a mut CurveFilletContactProducer<'a, E>,
    ) -> Self {
        Self {
            extensions: CurveExtensionOperations {
                extend: &mut *extensions.extend,
                trim: &mut *extensions.trim,
                clamp_endpoint: &mut *extensions.clamp_endpoint,
            },
            contacts,
        }
    }
}

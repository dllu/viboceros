//! Curve occurrences retain original owners while temporary pieces stay local.
use super::*;
use crate::uv_inputs::{Input, SubcurveInput};
use std::borrow::Cow;
pub(super) fn resolve<'a>(
    document: &'a Document,
    ranges: &[SubcurveInput],
) -> Result<Vec<Input<'a>>, CommandError> {
    let owners = ranges.iter().map(|r| r.object).collect::<BTreeSet<_>>();
    let mut inputs = document
        .selected_objects()
        .filter(|o| o.geometry().curve_ref().is_some() && !owners.contains(&o.id()))
        .map(|o| Input {
            object: o.id(),
            geometry: Cow::Borrowed(o.geometry()),
            temporary: false,
        })
        .collect::<Vec<_>>();
    for range in ranges {
        inputs.push(range.resolve_with_subcurve(document, USAGE, curve_segments::directed)?);
    }
    // Whole selections already follow document order. Only two effective
    // occurrences can execute, so avoid a document-sized ordering map.
    if !ranges.is_empty() && inputs.len() == 2 && inputs[0].object != inputs[1].object {
        let first = document
            .objects()
            .find(|o| o.id() == inputs[0].object || o.id() == inputs[1].object)
            .unwrap()
            .id();
        if first == inputs[1].object {
            inputs.swap(0, 1);
        }
    }
    Ok(inputs)
}

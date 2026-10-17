//! Shared exact curve and source-preservation witnesses.
use super::*;
pub(super) fn snapshot(
    doc: &Document,
    guides: &[(ObjectId, Geometry)],
) -> Result<Value, ProbeError> {
    snapshot_impl(doc, guides, false)
}
pub(super) fn snapshot_with_failed_lengths(
    doc: &Document,
    guides: &[(ObjectId, Geometry)],
) -> Result<Value, ProbeError> {
    snapshot_impl(doc, guides, true)
}
fn snapshot_impl(
    doc: &Document,
    guides: &[(ObjectId, Geometry)],
    allow_projective: bool,
) -> Result<Value, ProbeError> {
    let mut outputs = Vec::new();
    for object in doc
        .objects()
        .filter(|o| !guides.iter().any(|(id, _)| *id == o.id()))
    {
        let geometry = object.geometry();
        if let Geometry::Point(point) = geometry {
            outputs.push(json!({"selected":doc.is_selected(object.id()),"point":point.to_array()}));
            continue;
        }
        let curve = geometry
            .nurbs_curve_representation()?
            .ok_or(ProbeError::FixtureInvariant("primitive must be a curve"))?;
        let pieces = if let Geometry::PolyCurve(poly) = geometry {
            poly.segments()
                .iter()
                .map(|s| s.to_nurbs())
                .collect::<Result<Vec<_>, _>>()?
        } else {
            vec![curve.clone()]
        };
        let measured = geometry.curve_ref().unwrap().length(doc.tolerance());
        let (length, failed) = match measured {
            Ok(length) => (length, false),
            Err(_) if allow_projective && curve.quadratic_pole_fractions().is_some() => (0., true),
            Err(e) => return Err(e.into()),
        };
        let mut output = json!({"selected":doc.is_selected(object.id()),"curve":nurbs_curve_definition_value(&curve),"segments":pieces.iter().map(nurbs_curve_definition_value).collect::<Vec<_>>(),"length":length,"closed":curve.is_closed()?});
        if failed {
            output["length_query_failed"] = json!(true);
        }
        outputs.push(output);
    }
    Ok(
        json!({"outputs":outputs,"guide_unchanged":guides.iter().all(|(id,g)|doc.object(*id).is_some_and(|o|o.geometry()==g))}),
    )
}

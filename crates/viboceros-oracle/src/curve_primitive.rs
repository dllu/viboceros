//! Shared exact curve and source-preservation witnesses.
use super::*;
pub(super) fn snapshot(
    doc: &Document,
    guides: &[(ObjectId, Geometry)],
) -> Result<Value, ProbeError> {
    let mut outputs = Vec::new();
    for object in doc
        .objects()
        .filter(|o| !guides.iter().any(|(id, _)| *id == o.id()))
    {
        let geometry = object.geometry();
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
        outputs.push(json!({"selected":doc.is_selected(object.id()),"curve":nurbs_curve_definition_value(&curve),"segments":pieces.iter().map(nurbs_curve_definition_value).collect::<Vec<_>>(),"length":geometry.curve_ref().unwrap().length(doc.tolerance())?,"closed":curve.is_closed()?}));
    }
    Ok(
        json!({"outputs":outputs,"guide_unchanged":guides.iter().all(|(id,g)|doc.object(*id).is_some_and(|o|o.geometry()==g))}),
    )
}

//! Native ruled surface and base-cap construction with shared profile preparation.
use super::*;
use viboceros_smlib::{KernelCurve, Shape};

pub(super) fn extrude(
    curve: &NurbsCurve,
    apex: Point3,
    solid: bool,
    tolerance: Tolerance,
) -> Result<Geometry, CommandError> {
    let zero = Point3::try_new(0., 0., 0.)?;
    let local = curve.transformed(AffineTransform3::from_translation(apex.vector_to(zero)?))?;
    let profile = KernelCurve::from_nurbs_for_modeling(&local)?;
    let restore = AffineTransform3::from_translation(zero.vector_to(apex)?);
    if solid && curve.is_closed()? && curve.is_planar(tolerance)? {
        let output = Shape::apex_solid(&profile, zero, tolerance)?
            .to_brep(tolerance)?
            .transformed(restore, tolerance)?;
        Ok(Geometry::Brep(output))
    } else {
        Ok(Geometry::NurbsSurface(
            profile.apex_surface(zero)?.transformed(restore)?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn later_invalid_apex_profile_preserves_the_batch_and_redo() {
        let mut doc = Document::default();
        let registry = CommandRegistry::with_builtins();
        registry.execute(&mut doc, "Circle 0,0,0 2").unwrap();
        let first = doc.objects().next().unwrap().id();
        registry.execute(&mut doc, "Line 0,0,6 4,0,6").unwrap();
        let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
        let group = doc.add_group(None, ids.iter().copied()).unwrap();
        registry.execute(&mut doc, "Point 30,0,0").unwrap();
        registry.execute(&mut doc, "Undo").unwrap();
        doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
            .unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let members = doc.group(group).unwrap().members().collect::<Vec<_>>();
        assert!(
            registry
                .execute(
                    &mut doc,
                    "ExtrudeCrvToPoint 0,0,6 DeleteInput=Yes Solid=Yes"
                )
                .is_err()
        );
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(
            doc.group(group).unwrap().members().collect::<Vec<_>>(),
            members
        );
        assert_eq!(doc.redo_label(), Some("Point"));
        assert!(doc.is_selected(first));
    }
}

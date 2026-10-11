//! Native exact translational sweep and shared native end-cap construction.
use super::*;
use viboceros_smlib::{KernelCurve, Shape};

pub(super) fn extrude(
    curve: &NurbsCurve,
    path: &KernelCurve,
    solid: bool,
    tolerance: Tolerance,
) -> Result<Geometry, CommandError> {
    let origin = curve.evaluate(*curve.domain().start())?;
    let zero = Point3::try_new(0., 0., 0.)?;
    let local = curve.transformed(AffineTransform3::from_translation(origin.vector_to(zero)?))?;
    let profile = KernelCurve::from_nurbs_for_modeling(&local)?;
    let restore = AffineTransform3::from_translation(zero.vector_to(origin)?);
    if solid && curve.is_closed()? && curve.is_planar(tolerance)? {
        Ok(Geometry::Brep(
            Shape::translational_sweep(&profile, path, tolerance)?
                .to_brep(tolerance)?
                .transformed(restore, tolerance)?,
        ))
    } else {
        Ok(Geometry::NurbsSurface(
            profile.translational_sweep(path)?.transformed(restore)?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn later_coplanar_cap_preserves_the_batch_and_redo() {
        let mut doc = Document::default();
        let registry = CommandRegistry::with_builtins();
        registry.execute(&mut doc, "Circle 0,0,0 2").unwrap();
        let t = doc.tolerance();
        doc.add_geometry(Geometry::Circle(
            Circle3::try_new(
                Point3::try_new(8., 0., 0.).unwrap(),
                2.,
                UnitVector3::try_new(1., 0., 0., t).unwrap(),
                t,
            )
            .unwrap(),
        ))
        .unwrap();
        registry.execute(&mut doc, "Line 0,0,0 0,2,5").unwrap();
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
                .execute(&mut doc, "ExtrudeCrvAlongCrv DeleteInput=Yes Solid=Yes")
                .is_err()
        );
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(
            doc.group(group).unwrap().members().collect::<Vec<_>>(),
            members
        );
        assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), ids);
        assert_eq!(doc.redo_label(), Some("Point"));
    }
}

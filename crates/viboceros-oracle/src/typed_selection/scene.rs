//! Known geometry and local placement parity; no fitting or native selection data.
use super::*;
use viboceros_document::{BlockContent, BlockMember, BlockReference};

pub(super) fn build(doc: &mut Document, scenario: &str) -> Result<Vec<ObjectId>, ProbeError> {
    let t = doc.tolerance();
    let origin = Point3::try_new(0., 0., 0.)?;
    let p = |x, y, z| Point3::try_new(x, y, z);
    let frame = Frame3::try_from_normal(origin, Vector3::try_new(0., 0., 1.)?, t)?;
    let extrusion = Geometry::Brep(Brep::try_box_extrusion(frame, [[0., 2.], [0., 3.]], 4., t)?);
    let line = Geometry::Line(LineSegment::try_new(origin, p(2., 0., 0.)?, t)?);
    let mut ids = Vec::new();
    for geometry in [
        extrusion.clone(),
        Geometry::Brep(Brep::try_box(frame, [[0., 2.], [0., 3.], [0., 4.]], t)?),
        // Rhino's document object table wraps an inserted surface in a B-rep.
        Geometry::Brep(Brep::try_surface_face(
            NurbsSurface::try_bilinear([origin, p(2., 0., 0.)?, p(2., 3., 0.)?, p(0., 3., 0.)?])?,
            t,
        )?),
        line.clone(),
        Geometry::Point(p(13., 0., 0.)?),
    ] {
        ids.push(doc.add_geometry(geometry)?);
    }
    let attributes = ObjectAttributes::on_layer(doc.current_layer_id());
    let leaf = doc.add_block_definition(
        "Leaf",
        vec![
            BlockMember::new(BlockContent::Geometry(line.into()), attributes.clone()),
            BlockMember::new(BlockContent::Geometry(extrusion.into()), attributes.clone()),
        ],
    )?;
    let matrix = |x, y, z, shear| {
        AffineTransform3::try_new(
            [[x, shear, 0.], [0., y, 0.], [0., 0., z]],
            Vector3::try_new(0., 0., 0.)?,
        )
    };
    let identity = AffineTransform3::identity();
    let mirrored = matrix(-1., 1., 1., 0.)?;
    let nested = doc.add_block_definition(
        "Nested",
        vec![BlockMember::new(
            BlockContent::Reference(BlockReference::try_new(leaf, mirrored)?),
            attributes,
        )],
    )?;
    for (definition, placement) in [
        (leaf, identity),
        (leaf, mirrored),
        (leaf, matrix(-1., -1., 1., 0.)?),
        (leaf, matrix(-1., -1., -1., 0.)?),
        (nested, identity),
        (nested, mirrored),
        (leaf, matrix(1., -1., 1., 2.)?),
        (leaf, identity),
    ] {
        ids.push(doc.add_block_instance(BlockReference::try_new(definition, placement)?)?);
    }
    if scenario == "grouped" {
        doc.add_group(Some("Extrusion peers".into()), [ids[0], ids[3]])?;
        doc.add_group(Some("Instance peers".into()), [ids[5], ids[4]])?;
    }
    let targets = [ids[0], ids[5], ids[6]];
    if matches!(scenario, "hidden-layer" | "locked-layer") {
        let layer = doc.add_layer("Restricted", ColorRgb::BLACK)?;
        doc.set_objects_layer(targets, layer)?;
        if scenario == "hidden-layer" {
            doc.set_layer_visibility(layer, false)?;
        } else {
            doc.set_layer_locked(layer, true)?;
        }
    }
    if scenario == "hidden" {
        doc.set_objects_visibility(targets, false)?;
    }
    if scenario == "locked" {
        doc.set_objects_locked(targets, true)?;
    }
    if matches!(scenario, "no-matches" | "no-matches-preselected") {
        let hidden = std::iter::once(ids[0]).chain(ids[5..].iter().copied());
        doc.set_objects_visibility(hidden, false)?;
    }
    Ok(ids)
}

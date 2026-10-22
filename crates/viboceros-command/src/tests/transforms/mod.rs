//! transforms command regressions.
use super::*;

mod arrays;
mod basic;
mod curve_array;
mod orient;
mod orient_surface;
mod surface_array;

fn add_named_surface(document: &mut Document, surface: NurbsSurface, name: &str) -> ObjectId {
    document
        .add_geometry_with_attributes(
            Geometry::NurbsSurface(surface),
            ObjectAttributes::on_layer(document.current_layer_id()).with_name(name),
        )
        .unwrap()
}

fn orient_line(document: &Document, name: &str, excluded: &[ObjectId]) -> LineSegment {
    document
        .objects()
        .find(|object| object.attributes().name() == Some(name) && !excluded.contains(&object.id()))
        .map(|object| match object.geometry() {
            Geometry::Line(line) => *line,
            _ => panic!("expected an orient fixture line"),
        })
        .unwrap()
}

fn add_orient_triad(document: &mut Document) -> [ObjectId; 3] {
    let layer = document.current_layer_id();
    let tolerance = document.tolerance();
    let origin = Point3::try_new(1.0, 2.0, 3.0).unwrap();
    let mut ids = Vec::new();
    for (name, end) in [
        ("x", Point3::try_new(2.0, 2.0, 3.0).unwrap()),
        ("y", Point3::try_new(1.0, 3.0, 3.0).unwrap()),
        ("z", Point3::try_new(1.0, 2.0, 4.0).unwrap()),
    ] {
        ids.push(
            document
                .add_geometry_with_attributes(
                    Geometry::Line(LineSegment::try_new(origin, end, tolerance).unwrap()),
                    ObjectAttributes::on_layer(layer).with_name(name),
                )
                .unwrap(),
        );
    }
    let ids: [ObjectId; 3] = ids.try_into().unwrap();
    document.add_group(Some("Triad".to_owned()), ids).unwrap();
    document
        .select_object(ids[0], SelectionMode::Replace)
        .unwrap();
    ids
}

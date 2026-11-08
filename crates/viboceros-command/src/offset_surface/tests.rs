//! Existing analytic and planar offset regressions.
use super::*;

/// Aggregate separate offset skins only for the pre-existing SDK topology and
/// volume expectations. The command's object count and identity are checked
/// separately, including its unchanged selected source.
fn result_geometry(document: &Document) -> Geometry {
    let outputs = document
        .objects()
        .filter(|o| !document.is_selected(o.id()))
        .map(|o| o.geometry().clone())
        .collect::<Vec<_>>();
    match outputs.as_slice() {
        [] => document
            .selected_objects()
            .last()
            .unwrap()
            .geometry()
            .clone(),
        [one] => one.clone(),
        _ => Geometry::Brep(
            Brep::try_disjoint_union(
                outputs
                    .into_iter()
                    .map(|g| match g {
                        Geometry::Brep(b) => b,
                        Geometry::NurbsSurface(s) => {
                            Brep::try_surface_face(s, document.tolerance()).unwrap()
                        }
                        _ => panic!("offset skin"),
                    })
                    .collect(),
                document.tolerance(),
            )
            .unwrap(),
        ),
    }
}

mod cone;
mod cylinder;
mod planar;
mod sphere;
mod torus;

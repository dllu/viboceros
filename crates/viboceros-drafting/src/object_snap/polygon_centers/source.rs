//! Compare only data affecting Center targets, after snapshot identity changes.
use super::*;

pub(super) fn same_target_source(old: &Geometry, current: &Geometry) -> bool {
    match (old, current) {
        (Geometry::Brep(old), Geometry::Brep(current)) => {
            old.faces().len() == current.faces().len()
                && old.faces().iter().zip(current.faces()).all(|(a, b)| {
                    // Faces with holes have no polygon target. UV trim
                    // parameterization does not affect spatial polygon corners.
                    if a.loops().len() != 1 {
                        return b.loops().len() != 1;
                    }
                    b.loops().len() == 1
                        && a.surface() == b.surface()
                        && a.loops()[0].trims().len() == b.loops()[0].trims().len()
                        && a.loops()[0].trims().iter().zip(b.loops()[0].trims()).all(
                            |(a, b)| match (a.edge(), b.edge()) {
                                (None, None) => true,
                                (Some(i), Some(j)) => {
                                    old.edges()[i].curve() == current.edges()[j].curve()
                                        && a.is_reversed_3d() == b.is_reversed_3d()
                                }
                                _ => false,
                            },
                        )
                })
        }
        _ => old == current,
    }
}

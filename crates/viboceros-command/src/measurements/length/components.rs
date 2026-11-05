//! Prepare each component owner's topology or limit curves once per measurement.
use super::*;
use crate::{ComponentPick, ComponentSelectionKind as Kind};
use std::collections::BTreeMap;
pub(super) fn accumulate(
    doc: &Document,
    picks: Vec<ComponentPick>,
    add: &mut impl FnMut(f64) -> Result<(), CommandError>,
) -> Result<(), CommandError> {
    let mut owners = BTreeMap::new();
    for pick in picks {
        owners
            .entry(pick.object)
            .or_insert_with(Vec::new)
            .push(pick);
    }
    for object in doc.selectable_objects() {
        let Some(picks) = owners.remove(&object.id()) else {
            continue;
        };
        match object.geometry() {
            Geometry::Mesh(mesh) => {
                let edges = mesh.topology_edge_points();
                for p in picks {
                    if p.kind != Kind::MeshEdge {
                        return Err(CommandError::UnsupportedLengthGeometry);
                    }
                    let [a, b] = edges
                        .get(p.index)
                        .ok_or(CommandError::Usage("Select the mesh edges again"))?;
                    add(a.distance_to(*b)?)?;
                }
            }
            Geometry::SubD(subd) => {
                if picks.iter().any(|p| p.kind != Kind::SubDEdge) {
                    return Err(CommandError::UnsupportedLengthGeometry);
                }
                let indices = picks.iter().map(|p| p.index).collect::<Vec<_>>();
                if doc.subd_control_net() {
                    for index in indices {
                        let e = subd
                            .edges()
                            .get(index)
                            .ok_or(CommandError::Usage("Select the SubD edges again"))?;
                        add(subd.control_points()[e[0] as usize]
                            .distance_to(subd.control_points()[e[1] as usize])?)?;
                    }
                } else {
                    for curve in subd.edge_nurbs_curves(&indices)? {
                        add(viboceros_geometry::CurveRef::NurbsCurve(&curve)
                            .length(doc.tolerance())?)?;
                    }
                }
            }
            Geometry::Brep(_) | Geometry::NurbsSurface(_) => {
                for p in picks {
                    if p.kind != Kind::BrepEdge {
                        return Err(CommandError::UnsupportedLengthGeometry);
                    }
                    let curve = crate::curve_reference::resolve(
                        object.geometry(),
                        Some(p.index),
                        doc.tolerance(),
                    )
                    .ok_or(CommandError::UnsupportedLengthGeometry)?;
                    add(curve.curve().length(doc.tolerance())?)?;
                }
            }
            _ => return Err(CommandError::UnsupportedLengthGeometry),
        }
    }
    if !owners.is_empty() {
        return Err(CommandError::Usage("Select the edge components again"));
    }
    Ok(())
}

//! Experimental two-view preparation with immutable parents and atomic admission.
//!
//! This module is intentionally absent from the command registry: native curved
//! output fidelity has not passed the model gate. See `docs/curve-two-views.md`.
use super::*;
use curve_reference::CapturedCurve;
mod options;
pub use options::{Input, parse};
#[cfg(test)]
mod tests;
pub const USAGE: &str = "Crv2View [Sources=first-id,second-id] [Direction1=x,y,z] [Direction2=x,y,z] [Edge1=index] [Edge2=index] [SubCrv1=start,end] [SubCrv2=start,end]";

#[derive(Clone, Debug)]
pub struct Profile {
    pub object: ObjectId,
    pub edge: Option<usize>,
    pub parameters: Option<[Real; 2]>,
    pub wrap_edge: bool,
    pub direction: Option<Vector3>,
}
impl Profile {
    pub fn whole(object: ObjectId) -> Self {
        Self {
            object,
            edge: None,
            parameters: None,
            wrap_edge: false,
            direction: None,
        }
    }
    pub fn resolve(
        &self,
        doc: &Document,
        cplane: Frame3,
    ) -> Result<(CapturedCurve, NurbsCurve, Vector3), CommandError> {
        let source = CapturedCurve::capture(doc, self.object, self.edge)?;
        let curve = if let Some([a, b]) = self.parameters {
            let curve = source.curve_ref();
            let piece = if self.edge.is_some() && !self.wrap_edge {
                curve_reference::edge_subcurve(curve, a, b)?
            } else {
                prepare_directed_subcurve(&curve.to_owned(), a, b)?
            };
            piece.as_ref().to_nurbs()?
        } else {
            source.curve().clone()
        };
        let direction = if let Some(direction) = self.direction {
            direction
        } else {
            let points = curve
                .control_points()
                .iter()
                .map(|p| p.point())
                .collect::<Vec<_>>();
            viboceros_geometry::try_control_polygon_plane(&points, doc.tolerance())?
                .map_or(cplane.z_axis(), |p| p.normal())
                .as_vector()
        };
        Ok((source, curve, direction))
    }
}
#[derive(Debug)]
pub struct Prepared {
    parents: Vec<viboceros_document::Object>,
    tolerance: Tolerance,
    layer: viboceros_document::LayerId,
    output: Vec<NurbsCurve>,
}
impl Prepared {
    pub fn curves(&self) -> &[NurbsCurve] {
        &self.output
    }
    pub fn current(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance()
            && self.layer == doc.current_layer_id()
            && self.parents.iter().all(|old| {
                doc.is_object_selectable(old.id())
                    && doc.object(old.id()).is_some_and(|now| {
                        old == now
                            && old
                                .geometry_snapshot()
                                .shares_storage_with(now.geometry_snapshot())
                    })
            })
    }
    fn apply(&self, doc: &mut Document) -> Result<Vec<ObjectId>, CommandError> {
        if !self.current(doc) {
            return Err(CommandError::Usage(
                "Crv2View inputs or document settings changed; prepare again",
            ));
        }
        doc.clear_selection();
        self.output
            .iter()
            .cloned()
            .map(|c| {
                doc.add_geometry(Geometry::NurbsCurve(c))
                    .map_err(Into::into)
            })
            .collect()
    }
    pub fn commit(&self, doc: &mut Document) -> Result<Vec<ObjectId>, CommandError> {
        run_command_transaction(doc, "Crv2View", |d| self.apply(d))
    }
}
pub fn prepare(
    doc: &Document,
    profiles: &[Profile; 2],
    cplane: Frame3,
) -> Result<Prepared, CommandError> {
    let first = profiles[0].resolve(doc, cplane)?;
    let second = profiles[1].resolve(doc, cplane)?;
    let output = viboceros_geometry::curves_from_two_views_with_operation(
        [&first.1, &second.1],
        [first.2, second.2],
        doc.tolerance(),
        &mut curve_intersections::surface_pair_events,
    )?;
    if output.is_empty() {
        return Err(CommandError::Usage(
            "Crv2View profiles have no common spatial curve",
        ));
    }
    let parents = profiles
        .iter()
        .map(|p| doc.object(p.object).unwrap().clone())
        .collect();
    Ok(Prepared {
        parents,
        tolerance: doc.tolerance(),
        layer: doc.current_layer_id(),
        output,
    })
}

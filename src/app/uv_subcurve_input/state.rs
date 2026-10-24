//! Transient curve references, source validation and parent arguments.
use super::*;

#[derive(Clone, Debug)]
pub(in crate::app) struct CurveRange {
    pub(in crate::app) object: ObjectId,
    pub(in crate::app) parameters: [f64; 2],
    pub(in crate::app) source: viboceros_document::GeometrySnapshot,
    pub(in crate::app) edge_reference: Option<CapturedCurve>,
    pub(in crate::app) edge_wrap: bool,
}

impl CurveRange {
    pub(in crate::app) fn argument(&self) -> String {
        if let Some(edge) = self.edge_reference.as_ref().and_then(|r| r.source().1) {
            format!(
                " SubCrv={},{},{},{},{}",
                self.object,
                if self.edge_wrap { "EdgeWrap" } else { "Edge" },
                edge,
                self.parameters[0],
                self.parameters[1]
            )
        } else {
            format!(
                " SubCrv={},{},{}",
                self.object, self.parameters[0], self.parameters[1]
            )
        }
    }
    pub(in crate::app) fn current(&self, document: &Document) -> bool {
        self.edge_reference.as_ref().map_or_else(
            || {
                document.is_object_selectable(self.object)
                    && document
                        .object(self.object)
                        .is_some_and(|o| o.geometry_snapshot().shares_storage_with(&self.source))
            },
            |reference| reference.current(document),
        )
    }
}

#[derive(Clone, Debug, Default)]
pub(in crate::app) struct SubcurveInputs {
    pub(in crate::app) ranges: Vec<CurveRange>,
    pub(in crate::app) pending: Option<PendingSubcurve>,
    pub(in crate::app) extraction_order: Option<crate::app::extract_points::InputOrder>,
}

#[derive(Clone, Debug, Default)]
pub(in crate::app) struct PendingSubcurve {
    pub(in crate::app) object: Option<ObjectId>,
    pub(in crate::app) source: Option<viboceros_document::GeometrySnapshot>,
    pub(in crate::app) edge_reference: Option<CapturedCurve>,
    pub(in crate::app) start: Option<f64>,
    pub(in crate::app) length: Option<f64>,
    pub(in crate::app) hover_parameter: Option<f64>,
    pub(in crate::app) locked_forward: Option<bool>,
    pub(in crate::app) preview: crate::app::subcurve_preview::Cache,
}

impl PendingSubcurve {
    pub(in crate::app) fn edge(&self) -> Option<usize> {
        self.edge_reference.as_ref().and_then(|r| r.source().1)
    }
    pub(super) fn curve<'a>(
        &'a self,
        document: &'a Document,
    ) -> Option<viboceros_geometry::CurveRef<'a>> {
        self.edge_reference
            .as_ref()
            .map(|r| r.curve_ref())
            .or_else(|| {
                document
                    .object(self.object?)
                    .and_then(|o| o.geometry().curve_ref())
            })
    }
    pub(in crate::app) fn current(&self, document: &Document) -> bool {
        if let Some(reference) = &self.edge_reference {
            return reference.current(document);
        }
        self.object.is_some_and(|id| {
            document.is_object_selectable(id)
                && document.object(id).is_some_and(|o| {
                    self.source
                        .as_ref()
                        .is_some_and(|s| s.shares_storage_with(o.geometry_snapshot()))
                })
        })
    }
}

impl SubcurveInputs {
    pub(in crate::app) fn arguments(&self) -> String {
        self.ranges.iter().map(CurveRange::argument).collect()
    }
    pub(in crate::app) fn hint(&self) -> Option<&'static str> {
        if self
            .pending
            .as_ref()
            .is_some_and(|p| p.start.is_some() && p.locked_forward.is_some())
        {
            return Some("Pick the locked-side end or enter a length; Direction=Free unlocks");
        }
        self.pending.as_ref().map(|p| match (p.object, p.start,p.length) {
            (None, _,_) => "Select curve to temporarily shorten; Enter returns, Esc cancels",
            (Some(_), None,_) => "Pick start of temporary subcurve; Esc cancels",
            (Some(_), Some(_),None) => "Pick directed end or type a curve length; Esc cancels",
            (Some(_), Some(_),Some(_)) => "Confirm the length direction on the curve; type a new length or Enter to return",
        })
    }
}

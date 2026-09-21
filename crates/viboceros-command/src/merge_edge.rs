//! Component-driven edge merging: read-only choice preparation, one atomic replacement.
use super::*;
use viboceros_geometry::BrepEdgeMergeScope;

const USAGE: &str = "MergeEdge object-id edge-index [Edge|EdgeA|EdgeB|Both|All]";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergeEdgeChoice {
    Edge,
    EdgeA,
    EdgeB,
    Both,
    All,
}

impl MergeEdgeChoice {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
            "edge" => Some(Self::Edge),
            "edgea" => Some(Self::EdgeA),
            "edgeb" => Some(Self::EdgeB),
            "both" => Some(Self::Both),
            "all" => Some(Self::All),
            _ => None,
        }
    }
    pub const fn name(self) -> &'static str {
        match self {
            Self::Edge => "Edge",
            Self::EdgeA => "EdgeA",
            Self::EdgeB => "EdgeB",
            Self::Both => "Both",
            Self::All => "All",
        }
    }
}

/// A choice prompt owns a source snapshot so a stale component index cannot
/// silently edit different geometry after an external edit, Undo or replacement.
#[derive(Clone, Debug)]
pub struct MergeEdgeSelection {
    object: ObjectId,
    edge: usize,
    source: Geometry,
    brep: Brep,
    tolerance: Tolerance,
    available: [bool; 2],
}

impl MergeEdgeSelection {
    pub fn prepare(
        document: &Document,
        object: ObjectId,
        edge: usize,
    ) -> Result<Self, CommandError> {
        if !document.is_object_selectable(object) {
            return Err(CommandError::MergeEdgeUnavailable);
        }
        let source = document
            .object(object)
            .ok_or(CommandError::MergeEdgeUnavailable)?
            .geometry()
            .clone();
        let tolerance = document.tolerance();
        let brep = match &source {
            Geometry::Brep(brep) => brep.clone(),
            Geometry::NurbsSurface(surface) => Brep::try_surface_face(surface.clone(), tolerance)?,
            _ => return Err(CommandError::MergeEdgeUnavailable),
        };
        let mut available = [false; 2];
        for (i, scope) in [BrepEdgeMergeScope::Start, BrepEdgeMergeScope::End]
            .into_iter()
            .enumerate()
        {
            available[i] = brep
                .try_merge_edge_with_scope(edge, scope, edge_angle(tolerance), tolerance)?
                .edges()
                .len()
                < brep.edges().len();
        }
        Ok(Self {
            object,
            edge,
            source,
            brep,
            tolerance,
            available,
        })
    }

    pub fn choices(&self) -> &'static [MergeEdgeChoice] {
        use MergeEdgeChoice::*;
        match self.available {
            [true, true] => &[EdgeA, EdgeB, Both, All],
            [false, false] => &[],
            _ => &[Edge, All],
        }
    }

    pub fn object(&self) -> ObjectId {
        self.object
    }
    pub fn edge(&self) -> usize {
        self.edge
    }

    /// Spatial endpoints label the two immediate-neighbor choices in the UI.
    pub fn endpoints(&self) -> [Point3; 2] {
        self.brep.edges()[self.edge]
            .vertices()
            .map(|i| self.brep.vertices()[i].point())
    }

    /// Executes the prepared choice in one Undo record. No transaction is kept
    /// open while a user moves the camera, enters a choice, or cancels the prompt.
    pub fn commit(
        &self,
        document: &mut Document,
        choice: MergeEdgeChoice,
    ) -> Result<String, CommandError> {
        run_command_transaction(document, "MergeEdge", |document| {
            self.apply(document, choice)
        })
    }

    fn apply(
        &self,
        document: &mut Document,
        choice: MergeEdgeChoice,
    ) -> Result<String, CommandError> {
        if !document.is_object_selectable(self.object)
            || document.tolerance() != self.tolerance
            || document
                .object(self.object)
                .is_none_or(|o| o.geometry() != &self.source)
        {
            return Err(CommandError::MergeEdgeStale);
        }
        if !self.choices().contains(&choice) {
            return Err(CommandError::MergeEdgeChoiceUnavailable);
        }
        let scope = match choice {
            MergeEdgeChoice::Edge => {
                if self.available[0] {
                    BrepEdgeMergeScope::Start
                } else {
                    BrepEdgeMergeScope::End
                }
            }
            MergeEdgeChoice::EdgeA => BrepEdgeMergeScope::Start,
            MergeEdgeChoice::EdgeB => BrepEdgeMergeScope::End,
            MergeEdgeChoice::Both => BrepEdgeMergeScope::Both,
            MergeEdgeChoice::All => BrepEdgeMergeScope::Chain,
        };
        let merged = self.brep.try_merge_edge_with_scope(
            self.edge,
            scope,
            edge_angle(self.tolerance),
            self.tolerance,
        )?;
        let removed = self.brep.edges().len() - merged.edges().len();
        if removed == 0 {
            return Err(CommandError::MergeEdgeChoiceUnavailable);
        }
        // Unlike MergeAllEdges, measured MergeEdge replacements preserve kinky
        // faces. Component picks are transient, including on Undo and Redo.
        document.clear_selection();
        document.replace_object_geometries_with_history(
            [(self.object, Geometry::Brep(merged))],
            viboceros_document::ReplacementHistory::EveryReplacement,
        )?;
        Ok(format!("Merged {removed} redundant edge(s)"))
    }
}

fn edge_angle(tolerance: Tolerance) -> f64 {
    tolerance
        .angular()
        .clamp(0.1_f64.to_radians(), 1_f64.to_radians())
}

pub(super) struct MergeEdgeCommand;
impl Command for MergeEdgeCommand {
    fn name(&self) -> &'static str {
        "MergeEdge"
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if !(2..=3).contains(&arguments.len()) {
            return Err(CommandError::Usage(USAGE));
        }
        let object = arguments[0]
            .parse()
            .map_err(|_| CommandError::Usage(USAGE))?;
        let edge = arguments[1]
            .parse()
            .map_err(|_| CommandError::Usage(USAGE))?;
        let choice = arguments
            .get(2)
            .map_or(Some(MergeEdgeChoice::All), |choice| {
                MergeEdgeChoice::parse(choice)
            })
            .ok_or(CommandError::Usage(USAGE))?;
        MergeEdgeSelection::prepare(document, object, edge)?.apply(document, choice)
    }
}

#[cfg(test)]
mod tests;

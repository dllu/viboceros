//! Boolean picker policy, remembered options, and command formatting.
use viboceros_command::ObjectSelectionFilter;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum BooleanPromptKind {
    Intersection,
    Difference,
    Split,
    MeshIntersection(&'static str),
    MeshDifference(&'static str),
    MeshSplit(&'static str),
    MeshPartition(&'static str),
    MeshTrim,
}
impl BooleanPromptKind {
    pub(in crate::app) fn from_name(name: &str) -> Option<Self> {
        [
            Self::Intersection,
            Self::Difference,
            Self::Split,
            Self::MeshIntersection("Auto"),
            Self::MeshDifference("Auto"),
            Self::MeshSplit("Auto"),
            Self::MeshPartition("Auto"),
            Self::MeshTrim,
        ]
        .into_iter()
        .find(|kind| kind.name().eq_ignore_ascii_case(name))
    }
    pub(in crate::app) fn name(self) -> &'static str {
        match self {
            Self::Intersection => "BooleanIntersection",
            Self::Difference => "BooleanDifference",
            Self::Split => "BooleanSplit",
            Self::MeshIntersection(_) => "MeshBooleanIntersection",
            Self::MeshDifference(_) => "MeshBooleanDifference",
            Self::MeshSplit(_) => "MeshBooleanSplit",
            Self::MeshPartition(_) => "MeshSplit",
            Self::MeshTrim => "MeshTrim",
        }
    }
    pub(in crate::app) fn is_mesh_partition(self) -> bool {
        matches!(self, Self::MeshPartition(_))
    }
    pub(in crate::app) fn is_mesh_trim(self) -> bool {
        self == Self::MeshTrim
    }
    pub(in crate::app) fn filter_for(self, second: bool) -> ObjectSelectionFilter {
        if self.is_mesh_trim() {
            return if second {
                ObjectSelectionFilter::Mesh
            } else {
                ObjectSelectionFilter::MeshSplitCutters
            };
        }
        if self.is_mesh_partition() && second {
            ObjectSelectionFilter::MeshSplitCutters
        } else {
            self.filter()
        }
    }
    pub(in crate::app) fn is_intersection(self) -> bool {
        matches!(self, Self::Intersection | Self::MeshIntersection(_))
    }
    pub(in crate::app) fn is_difference(self) -> bool {
        matches!(self, Self::Difference | Self::MeshDifference(_))
    }
    pub(in crate::app) fn shares_sets(self) -> bool {
        self == Self::Split
    }
    pub(in crate::app) fn allows_empty_second(self) -> bool {
        self == Self::Intersection
    }
    pub(in crate::app) fn hint(self, second: bool) -> &'static str {
        match (self, second) {
            (Self::MeshTrim, false) => "Select cutting objects or IP; Enter continues, Esc cancels",
            (Self::MeshTrim, true) => {
                "Pick a mesh region to remove; Enter finishes, Undo reverses a pick"
            }
            (Self::Split, true) => "Select cutters; Enter splits, Esc cancels",
            (Self::Split, false) => "Select targets; Enter continues, Esc cancels",
            (Self::MeshPartition(_), true) => {
                "Select cutting objects or IP; Enter splits, Esc cancels"
            }
            (Self::MeshPartition(_), false) => {
                "Select objects to split; Enter continues, Esc cancels"
            }
            (Self::MeshSplit(_), true) => "Select cutting objects; Enter splits, Esc cancels",
            (Self::MeshSplit(_), false) => "Select objects to split; Enter continues, Esc cancels",
            (Self::Difference | Self::MeshDifference(_), true) => {
                "Select cutters; Enter subtracts, Esc cancels"
            }
            (Self::Difference | Self::MeshDifference(_), false) => {
                "Select targets; Enter continues, Esc cancels"
            }
            (Self::MeshIntersection(_), true) => {
                "Select second set; Enter executes the mesh Boolean, Esc cancels"
            }
            (Self::Intersection, true) => {
                "Select second set; empty Enter intersects the first set, Esc cancels"
            }
            (_, false) => "Select first set; Enter continues, Esc cancels",
        }
    }
    pub(in crate::app) fn filter(self) -> ObjectSelectionFilter {
        if self.is_mesh_trim() {
            ObjectSelectionFilter::MeshSplitCutters
        } else if self.mesh_tolerance().is_some() {
            ObjectSelectionFilter::EdgeAnalysis
        } else {
            ObjectSelectionFilter::SurfaceComponents
        }
    }
    pub(in crate::app) fn mesh_tolerance(self) -> Option<&'static str> {
        match self {
            Self::MeshIntersection(t)
            | Self::MeshDifference(t)
            | Self::MeshSplit(t)
            | Self::MeshPartition(t) => Some(t),
            _ => None,
        }
    }
    pub(in crate::app) fn with_mesh_tolerance(self, value: &'static str) -> Self {
        match self {
            Self::MeshIntersection(_) => Self::MeshIntersection(value),
            Self::MeshDifference(_) => Self::MeshDifference(value),
            Self::MeshSplit(_) => Self::MeshSplit(value),
            Self::MeshPartition(_) => Self::MeshPartition(value),
            _ => self,
        }
    }
}
#[derive(Clone, Debug)]
pub(in crate::app) struct BooleanOptions {
    pub(in crate::app) kind: BooleanPromptKind,
    pub(in crate::app) delete_input: bool,
    pub(in crate::app) delete_cutters: bool,
    pub(in crate::app) preselected_first: bool,
    pub(in crate::app) partition: Option<super::PartitionOptions>,
}
impl BooleanOptions {
    pub(in crate::app) fn shares_sets(&self) -> bool {
        self.kind.shares_sets() || self.partition.as_ref().is_some_and(|p| p.permit)
    }
    pub(in crate::app) fn prompt_line(&self, second: bool) -> String {
        if let Some(tolerance) = self.kind.mesh_tolerance()
            && self.kind.is_difference()
            && !second
        {
            format!("{} Tolerance={tolerance}", self.kind.name())
        } else {
            self.command_line(false)
        }
    }
    pub(in crate::app) fn command_line(&self, include_hidden: bool) -> String {
        if self.kind.is_mesh_trim() {
            return format!(
                "MeshTrim{}",
                self.partition
                    .as_ref()
                    .unwrap()
                    .planes
                    .iter()
                    .map(|p| format!(" InfinitePlane={p}"))
                    .collect::<String>()
            );
        }
        if let Some(tolerance) = self.kind.mesh_tolerance() {
            let mut command = format!("{} Tolerance={tolerance}", self.kind.name());
            if let Some(partition) = &self.partition {
                command.push_str(&partition.suffix(!include_hidden));
            }
            if self.kind.is_difference() {
                command.push_str(&format!(
                    " DeleteInput={}",
                    if self.delete_input { "Yes" } else { "No" }
                ));
            }
            return command;
        }
        let mut command = format!(
            "{} DeleteInput={}",
            self.kind.name(),
            if self.delete_input { "Yes" } else { "No" }
        );
        if self.kind == BooleanPromptKind::Difference && (include_hidden || self.delete_input) {
            command.push_str(&format!(
                " DeleteCutters={}",
                if self.delete_cutters { "Yes" } else { "No" }
            ));
        }
        command
    }
}

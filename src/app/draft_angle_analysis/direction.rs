//! Viewport direction controls and nonmutating point/curve getters.
use super::*;
use viboceros_geometry::Vector3;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::app) enum Source {
    WorldZ,
    #[default]
    CPlaneZ,
    View,
    TwoPoints,
    Curve,
    NamedCPlane(String),
}
impl Source {
    pub(super) const STANDARD: [Self; 5] = [
        Self::CPlaneZ,
        Self::WorldZ,
        Self::View,
        Self::TwoPoints,
        Self::Curve,
    ];
    pub(super) fn label(&self) -> &str {
        match self {
            Self::WorldZ => "World Z",
            Self::CPlaneZ => "CPlane Z",
            Self::View => "View based",
            Self::TwoPoints => "Two points",
            Self::Curve => "Curve or edge",
            Self::NamedCPlane(name) => name,
        }
    }
}

impl VibocerosApp {
    pub(in crate::app) fn picking_draft_pull_curve(&self) -> bool {
        matches!(
            self.active_command,
            Some(InteractiveCommand::DraftPullCurve)
        )
    }
    pub(in crate::app) fn picking_draft_pull(&self) -> bool {
        self.picking_draft_pull_curve()
            || matches!(
                self.active_command,
                Some(InteractiveCommand::DraftPullPoints { .. })
            )
    }
    fn apply_draft_pull_vector(&mut self, up: Vector3) {
        let [x, y, z] = up.to_array();
        self.set_analysis_display(&format!("DraftAngleAnalysis Up={x},{y},{z}"));
    }
    pub(in crate::app) fn set_draft_pull_source(&mut self, source: Source) {
        if self
            .commands
            .draft_angle_analysis_view(&self.document)
            .is_none()
        {
            self.push_log("Start DraftAngleAnalysis before setting its pull direction".into());
            return;
        }
        let result = (|| -> Result<Option<Vector3>, String> {
            Ok(match &source {
                Source::WorldZ => Some(Vector3::try_from([0., 0., 1.]).unwrap()),
                Source::CPlaneZ => Some(
                    self.viewports[self.active_viewport]
                        .construction_plane()
                        .z_axis()
                        .as_vector(),
                ),
                Source::View => Some(
                    self.viewports[self.active_viewport]
                        .camera_direction()
                        .scaled(-1.)
                        .map_err(|e| e.to_string())?,
                ),
                Source::NamedCPlane(name) => Some(
                    self.named_cplanes
                        .get(name)
                        .map_err(|e| e.to_string())?
                        .plane
                        .z_axis()
                        .as_vector(),
                ),
                Source::TwoPoints | Source::Curve => None,
            })
        })();
        let up = match result {
            Ok(up) => up,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return;
            }
        };
        if up.is_none() || self.picking_draft_pull() {
            self.cancel_interactive_command(false);
        }
        self.draft_pull_source = source.clone();
        if let Some(up) = up {
            self.apply_draft_pull_vector(up);
        } else {
            let command = if source == Source::Curve {
                InteractiveCommand::DraftPullCurve
            } else {
                InteractiveCommand::DraftPullPoints { start: None }
            };
            self.active_command = Some(command);
            self.command_input.clear();
            self.push_log(command.prompt().into());
        }
    }
    pub(in crate::app) fn accept_draft_pull_point(
        &mut self,
        start: Option<Point3>,
        point: Point3,
    ) -> bool {
        if let Some(start) = start {
            match start.vector_to(point).and_then(|up| {
                up.normalized_nonzero()?;
                Ok(up)
            }) {
                Ok(up) => {
                    self.cancel_interactive_command(false);
                    self.apply_draft_pull_vector(up);
                }
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    return false;
                }
            }
        } else {
            let command = InteractiveCommand::DraftPullPoints { start: Some(point) };
            self.drafting_plane = Some(self.viewports[self.active_viewport].construction_plane());
            self.active_command = Some(command);
            self.push_log(command.prompt().into());
        }
        true
    }
    pub(in crate::app) fn pick_draft_pull_curve(
        &mut self,
        id: Option<ObjectId>,
        edge: Option<usize>,
    ) -> bool {
        if !self.picking_draft_pull_curve() {
            return false;
        }
        let result = (|| -> Result<Vector3, String> {
            let id = id.ok_or("Select a curve or surface edge")?;
            let object = self
                .document
                .object(id)
                .filter(|_| self.document.is_object_selectable(id))
                .ok_or("Reference is hidden, locked, or missing")?;
            let curve = viboceros_command::curve_reference::resolve(
                object.geometry(),
                edge,
                self.document.tolerance(),
            )
            .ok_or("Select a curve or surface edge")?;
            let curve = curve.curve();
            Ok(curve
                .evaluate_with_tangent(*curve.domain().start())
                .map_err(|e| e.to_string())?
                .tangent()
                .as_vector())
        })();
        match result {
            Ok(up) => {
                self.cancel_interactive_command(false);
                self.apply_draft_pull_vector(up);
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }
    pub(in crate::app) fn try_draft_pull_command(&mut self, input: &str) -> bool {
        let Some((command, tail)) = input.trim().split_once(char::is_whitespace) else {
            return false;
        };
        if !command
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("DraftAngleAnalysis")
        {
            return false;
        }
        let tail = tail.trim_start();
        let (option, rest) = tail.split_once(char::is_whitespace).unwrap_or((tail, ""));
        let Some((key, value)) = option.split_once('=') else {
            return false;
        };
        if !key
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("Direction")
        {
            return false;
        }
        let rest = rest.trim();
        let source = match value.to_ascii_lowercase().as_str() {
            "worldz" if rest.is_empty() => Some(Source::WorldZ),
            "cplanez" if rest.is_empty() => Some(Source::CPlaneZ),
            "view" if rest.is_empty() => Some(Source::View),
            "twopoints" if rest.is_empty() => Some(Source::TwoPoints),
            "curve" if rest.is_empty() => Some(Source::Curve),
            "namedcplane" if !rest.is_empty() => {
                Some(Source::NamedCPlane(rest.trim_matches('"').to_owned()))
            }
            _ => None,
        };
        self.command_input.clear();
        if let Some(source) = source {
            self.set_draft_pull_source(source);
        } else {
            self.push_log("DraftAngleAnalysis Direction=WorldZ|CPlaneZ|View|TwoPoints|Curve|NamedCPlane [name]".into());
        }
        true
    }
}

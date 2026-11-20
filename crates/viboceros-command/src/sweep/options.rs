//! Shared script and interactive Sweep1 options; updates validate atomically.
use super::*;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Options {
    pub rail_name: Option<String>,
    pub rail_id: Option<ObjectId>,
    pub rail_edge: Option<RailReference>,
    pub parameters: Option<Vec<Real>>,
    pub roadlike: Option<bool>,
    pub surface_aligned: bool,
    pub axis: Option<UnitVector3>,
    pub global: Option<bool>,
    pub refit: Option<bool>,
    pub closed: Option<bool>,
    pub untrimmed_miters: Option<bool>,
    pub rebuild_point_count: usize,
    /// The last count selected, including when rebuilding is inactive.
    pub saved_rebuild_count: Option<usize>,
    pub refit_tolerance: Option<Real>,
    /// The last tolerance selected, including when fitting is inactive.
    pub saved_refit_tolerance: Option<Real>,
    pub ends: SweepEndPoints,
}

#[derive(Clone, Copy)]
enum Simplify {
    None,
    Rebuild,
    Refit,
}

impl Options {
    pub fn rebuild_count(&self) -> usize {
        self.saved_rebuild_count
            .or_else(|| (self.rebuild_point_count != 0).then_some(self.rebuild_point_count))
            .unwrap_or(5)
    }

    pub fn refit_tolerance(&self) -> Real {
        self.saved_refit_tolerance
            .or(self.refit_tolerance)
            .unwrap_or(0.01)
    }

    fn simplification(&self) -> Simplify {
        if self.rebuild_point_count != 0 {
            Simplify::Rebuild
        } else if self.refit_tolerance.is_some() {
            Simplify::Refit
        } else {
            Simplify::None
        }
    }

    /// Value getters accepted by both scripted and interactive Sweep1 input.
    pub const VALUE_OPTIONS: &[&str] = &[
        "RailName",
        "RailId",
        "RailEdge",
        "Parameters",
        "FrameStyle",
        "Style",
        "Axis",
        "GlobalShapeBlending",
        "ShapeBlending",
        "RefitRail",
        "Closed",
        "UntrimmedMiters",
        "Rebuild",
        "RebuildPointCount",
        "RebuildCount",
        "Refit",
        "RefitTolerance",
        "Simplify",
    ];

    pub fn parse(arguments: &[&str]) -> Result<Self, CommandError> {
        Self::default().updated(arguments)
    }

    /// Replace the supplied choices, rejecting duplicate keys in this update.
    pub fn updated(&self, arguments: &[&str]) -> Result<Self, CommandError> {
        let mut result = self.clone();
        let mut seen = 0u16;
        let mut simplify = None;
        let mut mode_from_value = None;
        for argument in arguments {
            let (name, value) = argument.split_once('=').ok_or(CommandError::Usage(USAGE))?;
            let option = name.trim_start_matches(['_', '-']).to_ascii_lowercase();
            let key = match option.as_str() {
                "railname" if !value.is_empty() => {
                    result.rail_name = Some(value.into());
                    result.rail_id = None;
                    result.rail_edge = None;
                    1
                }
                "railid" => {
                    result.rail_id = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?);
                    result.rail_name = None;
                    result.rail_edge = None;
                    1
                }
                "railedge" => {
                    result.rail_edge = Some(RailReference::parse_edge(value)?);
                    result.rail_id = None;
                    result.rail_name = None;
                    1
                }
                "parameters" => {
                    result.parameters = Some(
                        value
                            .split(',')
                            .map(parse_finite_real)
                            .collect::<Result<Vec<_>, _>>()?,
                    );
                    2
                }
                "framestyle" | "style" => {
                    let (roadlike, aligned) =
                        match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                            "freeform" => (false, false),
                            "roadlike" => (true, false),
                            "surface" | "alignwithsurface" => (false, true),
                            _ => return Err(CommandError::Usage(USAGE)),
                        };
                    result.roadlike = Some(roadlike);
                    result.surface_aligned = aligned;
                    4
                }
                "axis" => {
                    result.axis = Some(
                        Vector3::try_from(parse_single_option_point(value, USAGE)?.to_array())?
                            .normalized_nonzero()?,
                    );
                    8
                }
                "globalshapeblending" => {
                    result.global = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
                    16
                }
                "shapeblending" => {
                    result.global = Some(
                        match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                            "local" => false,
                            "global" => true,
                            _ => return Err(CommandError::Usage(USAGE)),
                        },
                    );
                    16
                }
                "refitrail" => {
                    result.refit = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
                    32
                }
                "startpoint" => {
                    result.ends.start = Some(parse_single_option_point(value, USAGE)?);
                    64
                }
                "endpoint" => {
                    result.ends.end = Some(parse_single_option_point(value, USAGE)?);
                    128
                }
                "rebuild" | "rebuildpointcount" | "rebuildcount" => {
                    let sets_mode = option != "rebuildcount";
                    let count = value
                        .parse::<usize>()
                        .ok()
                        .filter(|n| {
                            (*n == 0 && sets_mode || *n >= 3) && *n <= Sweep1::MAX_SECTION_CONTROLS
                        })
                        .ok_or(CommandError::Usage(USAGE))?;
                    if count != 0 {
                        result.saved_rebuild_count = Some(count);
                    }
                    if sets_mode {
                        if mode_from_value.is_some() {
                            return Err(CommandError::Usage(USAGE));
                        }
                        mode_from_value = Some(if count == 0 {
                            Simplify::None
                        } else {
                            Simplify::Rebuild
                        });
                    }
                    256
                }
                "refit" | "refittolerance" => {
                    let tolerance = parse_finite_real(value)?;
                    if tolerance <= 0. {
                        return Err(CommandError::Usage(USAGE));
                    }
                    result.saved_refit_tolerance = Some(tolerance);
                    if option == "refit" {
                        if mode_from_value.is_some() {
                            return Err(CommandError::Usage(USAGE));
                        }
                        mode_from_value = Some(Simplify::Refit);
                    }
                    4096
                }
                "simplify" => {
                    simplify = Some(
                        match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                            "none" => Simplify::None,
                            "rebuild" => Simplify::Rebuild,
                            "refit" => Simplify::Refit,
                            _ => return Err(CommandError::Usage(USAGE)),
                        },
                    );
                    512
                }
                "closed" => {
                    result.closed = Some(
                        parse_yes_no(value.trim_start_matches('_'))
                            .ok_or(CommandError::Usage(USAGE))?,
                    );
                    1024
                }
                "untrimmedmiters" => {
                    result.untrimmed_miters =
                        Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
                    2048
                }
                _ => return Err(CommandError::Usage(USAGE)),
            };
            if seen & key != 0 {
                return Err(CommandError::Usage(USAGE));
            }
            seen |= key;
        }
        if seen & 4 != 0 && seen & 8 == 0 {
            result.axis = None;
        }
        if result.axis.is_some() && !result.roadlike.unwrap_or(false) {
            return Err(CommandError::Usage(USAGE));
        }
        let mode = simplify
            .or(mode_from_value)
            .unwrap_or(self.simplification());
        let count = result.rebuild_count();
        let tolerance = result.refit_tolerance();
        result.rebuild_point_count = 0;
        result.refit_tolerance = None;
        match mode {
            Simplify::None => {}
            Simplify::Rebuild => {
                result.saved_rebuild_count = Some(count);
                result.rebuild_point_count = count;
            }
            Simplify::Refit => {
                result.saved_refit_tolerance = Some(tolerance);
                result.refit_tolerance = Some(tolerance);
            }
        }
        Ok(result)
    }

    /// Resolve an explicit id or unique case-insensitive name using the command's
    /// lookup policy. The caller still validates curve type and selectability.
    pub fn explicit_rail(&self, document: &Document) -> Result<Option<ObjectId>, CommandError> {
        if let Some(id) = self.rail_id {
            return Ok(Some(id));
        }
        self.rail_name
            .as_deref()
            .map(|name| {
                extrude_along_curve::resolve_curve_along_curve_path(document, &[], Some(name))
            })
            .transpose()
    }

    pub fn has_explicit_rail(&self) -> bool {
        self.rail_name.is_some() || self.rail_id.is_some() || self.rail_edge.is_some()
    }

    pub fn rail_reference(
        &self,
        document: &Document,
    ) -> Result<Option<RailReference>, CommandError> {
        if let Some(edge) = self.rail_edge {
            return Ok(Some(edge));
        }
        Ok(self.explicit_rail(document)?.map(|id| RailReference {
            target: Target::Curve(id),
            face: None,
        }))
    }

    /// Canonical command arguments for the shared command executor.
    pub fn arguments(&self) -> String {
        let mut words = Vec::new();
        match self.simplification() {
            Simplify::Rebuild => words.push(format!("Rebuild={}", self.rebuild_point_count)),
            Simplify::Refit => words.push(format!("Refit={}", self.refit_tolerance())),
            Simplify::None
                if self.saved_rebuild_count.is_some() || self.saved_refit_tolerance.is_some() =>
            {
                words.push("Simplify=None".into());
            }
            Simplify::None => {}
        }
        if self.rebuild_point_count == 0
            && let Some(count) = self.saved_rebuild_count
        {
            words.push(format!("RebuildCount={count}"));
        }
        if self.refit_tolerance.is_none()
            && let Some(tolerance) = self.saved_refit_tolerance
        {
            words.push(format!("RefitTolerance={tolerance}"));
        }
        if let Some(name) = &self.rail_name {
            words.push(format!("RailName={name}"));
        }
        if let Some(id) = self.rail_id {
            words.push(format!("RailId={id}"));
        }
        if let Some(RailReference {
            target: Target::Edge(id, edge),
            face,
        }) = self.rail_edge
        {
            let mut option = format!("RailEdge={id},{edge}");
            if let Some(face) = face {
                option.push_str(&format!(",{face}"));
            }
            words.push(option);
        }
        if let Some(parameters) = &self.parameters {
            words.push(format!(
                "Parameters={}",
                parameters
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        if let Some(roadlike) = self.roadlike {
            words.push(format!(
                "FrameStyle={}",
                if self.surface_aligned {
                    "AlignWithSurface"
                } else if roadlike {
                    "Roadlike"
                } else {
                    "Freeform"
                }
            ));
        }
        let coordinates = |p: [Real; 3]| {
            p.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        };
        if let Some(axis) = self.axis {
            words.push(format!("Axis={}", coordinates(axis.as_vector().to_array())));
        }
        if let Some(global) = self.global {
            words.push(
                if global {
                    "ShapeBlending=Global"
                } else {
                    "ShapeBlending=Local"
                }
                .into(),
            );
        }
        if let Some(refit) = self.refit {
            words.push(format!("RefitRail={}", if refit { "Yes" } else { "No" }));
        }
        if let Some(closed) = self.closed {
            words.push(format!("Closed={}", if closed { "Yes" } else { "No" }));
        }
        if let Some(untrimmed) = self.untrimmed_miters {
            words.push(format!(
                "UntrimmedMiters={}",
                if untrimmed { "Yes" } else { "No" }
            ));
        }
        for (name, value) in [("StartPoint", self.ends.start), ("EndPoint", self.ends.end)] {
            if let Some(value) = value {
                words.push(format!("{name}={}", coordinates(value.to_array())));
            }
        }
        words.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rebuild_modes_share_aliases_and_atomic_option_updates() {
        let source = Options::parse(&["Simplify=Rebuild"]).unwrap();
        assert_eq!(source.rebuild_point_count, 5);
        for name in ["Rebuild", "RebuildCount", "RebuildPointCount"] {
            let next = source.updated(&[&format!("{name}=7")]).unwrap();
            assert_eq!(next.rebuild_point_count, 7);
            assert_eq!(
                Options::parse(&next.arguments().split_whitespace().collect::<Vec<_>>()).unwrap(),
                next
            );
            assert!(Options::VALUE_OPTIONS.contains(&name));
        }
        for bad in [
            "Rebuild=1",
            "Rebuild=2",
            "Rebuild=513",
            "Rebuild=-1",
            "Rebuild=NaN",
            "Simplify=Other",
            "Rebuild=5 RebuildCount=6",
        ] {
            assert!(
                source
                    .updated(&bad.split_whitespace().collect::<Vec<_>>())
                    .is_err()
            );
            assert_eq!(source.rebuild_point_count, 5);
        }
        assert_eq!(
            source
                .updated(&["Simplify=None"])
                .unwrap()
                .rebuild_point_count,
            0
        );
        assert_eq!(
            source.updated(&["Rebuild=0"]).unwrap().rebuild_point_count,
            0
        );
    }
    #[test]
    fn rebuild_count_survives_disabling_and_can_be_edited_while_inactive() {
        let rebuilt = Options::parse(&["Rebuild=9"]).unwrap();
        let inactive = rebuilt.updated(&["Simplify=None"]).unwrap();
        assert_eq!(inactive.rebuild_count(), 9);
        assert_eq!(inactive.rebuild_point_count, 0);
        let edited = inactive.updated(&["RebuildCount=8"]).unwrap();
        assert_eq!(edited.rebuild_count(), 8);
        assert_eq!(edited.rebuild_point_count, 0);
        assert_eq!(
            Options::parse(&edited.arguments().split_whitespace().collect::<Vec<_>>()).unwrap(),
            edited
        );
        assert_eq!(
            edited
                .updated(&["Simplify=Rebuild"])
                .unwrap()
                .rebuild_point_count,
            8
        );
        assert!(edited.updated(&["RebuildCount=0"]).is_err());
    }

    #[test]
    fn refit_options_retain_both_simplification_values_and_independent_rail_refit() {
        let rebuilt =
            Options::parse(&["Rebuild=8", "RefitRail=No", "RefitTolerance=0.04"]).unwrap();
        assert_eq!(rebuilt.rebuild_point_count, 8);
        assert_eq!(rebuilt.refit_tolerance, None);
        let refitted = rebuilt.updated(&["Simplify=_Refit"]).unwrap();
        assert_eq!(refitted.rebuild_point_count, 0);
        assert_eq!(refitted.refit_tolerance, Some(0.04));
        assert_eq!(refitted.refit, Some(false));
        let off = refitted.updated(&["Simplify=None"]).unwrap();
        assert_eq!(off.refit_tolerance, None);
        assert_eq!(off.refit_tolerance(), 0.04);
        assert_eq!(off.rebuild_count(), 8);
        for options in [rebuilt, refitted, off] {
            assert_eq!(
                Options::parse(&options.arguments().split_whitespace().collect::<Vec<_>>())
                    .unwrap(),
                options
            );
            for bad in [
                "Refit=-1",
                "RefitTolerance=0",
                "RefitTolerance=NaN",
                "RefitTolerance=inf",
                "Refit=0.01 RefitTolerance=0.02",
                "Refit=0.01 Rebuild=5",
            ] {
                assert!(
                    options
                        .updated(&bad.split_whitespace().collect::<Vec<_>>())
                        .is_err()
                );
            }
        }
    }

    #[test]
    fn updating_points_preserves_other_choices_and_serializes_for_execution() {
        let source = Options::parse(&[
            "RailName=Rail",
            "Parameters=1,4",
            "FrameStyle=Roadlike",
            "Axis=0,1,0",
            "GlobalShapeBlending=No",
            "RefitRail=No",
        ])
        .unwrap();
        let result = source
            .updated(&["StartPoint=0,0,0", "EndPoint=1,0,5"])
            .unwrap();
        assert!(source.ends.is_empty());
        let mut without_ends = result.clone();
        without_ends.ends = SweepEndPoints::default();
        assert_eq!(source, without_ends);
        assert_eq!(
            Options::parse(&result.arguments().split_whitespace().collect::<Vec<_>>()).unwrap(),
            result
        );
    }
    #[test]
    fn option_updates_are_atomic_and_rail_choices_replace_one_another() {
        let source = Options::parse(&["RailName=Old", "StartPoint=0,0,0"]).unwrap();
        let copy = source.clone();
        for bad in [
            &["RefitRail=Yes", "RefitRail=No"][..],
            &["StartPoint=0,0,1", "StartPoint=0,0,2"],
            &["RailName=A", "RailId=bad"],
            &["StartPoint=NaN,0,0"],
            &["Unknown=No"],
            &["GlobalShapeBlending=Yes", "ShapeBlending=Local"],
        ] {
            assert!(source.updated(bad).is_err());
            assert_eq!(source, copy);
        }
        let id: ObjectId = "6f55f89c-712e-4ebd-a29c-8cd4ed8db70c".parse().unwrap();
        let text = format!("RailId={id}");
        let by_id = source.updated(&[&text]).unwrap();
        assert_eq!(by_id.rail_name, None);
        assert_eq!(by_id.rail_id, Some(id));
        let named = by_id.updated(&["RailName=New"]).unwrap();
        assert_eq!(named.rail_id, None);
        assert_eq!(named.rail_name.as_deref(), Some("New"));
    }
    #[test]
    fn switching_to_freeform_clears_only_a_previously_saved_axis() {
        let road = Options::parse(&["FrameStyle=Roadlike", "Axis=1,0,0"]).unwrap();
        let free = road.updated(&["FrameStyle=Freeform"]).unwrap();
        assert_eq!(free.axis, None);
        assert_eq!(road.updated(&["FrameStyle=Roadlike"]).unwrap().axis, None);
        assert!(
            road.updated(&["FrameStyle=Freeform", "Axis=0,1,0"])
                .is_err()
        );
        assert!(Options::parse(&["Axis=0,1,0"]).is_err());
    }
}

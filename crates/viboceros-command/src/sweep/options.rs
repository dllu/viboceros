//! Shared script and interactive Sweep1 options; updates validate atomically.
use super::*;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Options {
    pub rail_name: Option<String>,
    pub rail_id: Option<ObjectId>,
    pub parameters: Option<Vec<Real>>,
    pub roadlike: Option<bool>,
    pub axis: Option<UnitVector3>,
    pub global: Option<bool>,
    pub refit: Option<bool>,
    pub ends: SweepEndPoints,
}

impl Options {
    pub fn parse(arguments: &[&str]) -> Result<Self, CommandError> {
        Self::default().updated(arguments)
    }

    /// Replace the supplied choices, rejecting duplicate keys in this update.
    pub fn updated(&self, arguments: &[&str]) -> Result<Self, CommandError> {
        let mut result = self.clone();
        let mut seen = 0u16;
        for argument in arguments {
            let (name, value) = argument.split_once('=').ok_or(CommandError::Usage(USAGE))?;
            let key = match name
                .trim_start_matches(['_', '-'])
                .to_ascii_lowercase()
                .as_str()
            {
                "railname" if !value.is_empty() => {
                    result.rail_name = Some(value.into());
                    result.rail_id = None;
                    1
                }
                "railid" => {
                    result.rail_id = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?);
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
                "framestyle" => {
                    result.roadlike = Some(
                        match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                            "freeform" => false,
                            "roadlike" => true,
                            _ => return Err(CommandError::Usage(USAGE)),
                        },
                    );
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
                _ => return Err(CommandError::Usage(USAGE)),
            };
            if seen & key != 0 {
                return Err(CommandError::Usage(USAGE));
            }
            seen |= key;
        }
        if seen & 4 != 0 && result.roadlike == Some(false) && seen & 8 == 0 {
            result.axis = None;
        }
        if result.axis.is_some() && !result.roadlike.unwrap_or(false) {
            return Err(CommandError::Usage(USAGE));
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

    /// Canonical command arguments for the shared command executor.
    pub fn arguments(&self) -> String {
        let mut words = Vec::new();
        if let Some(name) = &self.rail_name {
            words.push(format!("RailName={name}"));
        }
        if let Some(id) = self.rail_id {
            words.push(format!("RailId={id}"));
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
                if roadlike { "Roadlike" } else { "Freeform" }
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
        for (name, value) in [
            ("GlobalShapeBlending", self.global),
            ("RefitRail", self.refit),
        ] {
            if let Some(value) = value {
                words.push(format!("{name}={}", if value { "Yes" } else { "No" }));
            }
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
        assert!(
            road.updated(&["FrameStyle=Freeform", "Axis=0,1,0"])
                .is_err()
        );
        assert!(Options::parse(&["Axis=0,1,0"]).is_err());
    }
}

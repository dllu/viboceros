//! OffsetMesh preferences are published only after a successful offset.
use viboceros_geometry::Real;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Method {
    Normals,
    Vector,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) struct Preferences {
    pub(super) distance: Real,
    pub(super) method: Method,
    pub(super) average_normals: bool,
    pub(super) use_vector_length: bool,
    pub(super) dynamic_preview: bool,
    pub(super) delete_input: bool,
    pub(super) solid: bool,
    pub(super) both_sides: bool,
    pub(super) allow_disjoint: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            distance: 1.0,
            method: Method::Normals,
            average_normals: false,
            use_vector_length: false,
            dynamic_preview: true,
            delete_input: false,
            solid: false,
            both_sides: false,
            allow_disjoint: false,
        }
    }
}

pub(super) fn option_parts(token: &str) -> (String, Option<&str>) {
    let token = token.trim_start_matches('_');
    let (key, value) = token.split_once('=').map_or((token, None), |(key, value)| {
        (key, Some(value.trim_start_matches('_')))
    });
    (key.to_ascii_lowercase(), value)
}

pub(super) fn toggle(value: Option<&str>, previous: bool) -> Result<bool, &'static str> {
    match value {
        None => Ok(!previous),
        Some(value) if value.eq_ignore_ascii_case("Yes") => Ok(true),
        Some(value) if value.eq_ignore_ascii_case("No") => Ok(false),
        _ => Err("Enter Yes or No"),
    }
}

impl Preferences {
    pub(super) fn set_method(&mut self, value: &str) -> Result<(), &'static str> {
        self.method = if value.eq_ignore_ascii_case("UseVertexNormals") {
            Method::Normals
        } else if value.eq_ignore_ascii_case("UserSelectedDirection") {
            Method::Vector
        } else {
            return Err("Choose UseVertexNormals or UserSelectedDirection");
        };
        Ok(())
    }

    pub(super) fn set_toggle(
        &mut self,
        name: &str,
        value: Option<&str>,
    ) -> Option<Result<(), &'static str>> {
        let setting = match name {
            "averagenormals" if self.method == Method::Normals => &mut self.average_normals,
            "usevectorlength" if self.method == Method::Vector => &mut self.use_vector_length,
            "dynamicpreview" => &mut self.dynamic_preview,
            "deleteinput" => &mut self.delete_input,
            "solid" => &mut self.solid,
            "bothsides" => &mut self.both_sides,
            "allowdisjoint" => &mut self.allow_disjoint,
            _ => return None,
        };
        Some(toggle(value, *setting).map(|value| *setting = value))
    }

    pub(super) fn prompt(self) -> String {
        let yes_no = |flag| if flag { "Yes" } else { "No" };
        let (direction_prompt, method, direction_options) = match self.method {
            Method::Normals => (
                "Offset distance",
                "UseVertexNormals",
                format!(
                    "AverageNormals={} Flip FlipAll",
                    yes_no(self.average_normals)
                ),
            ),
            Method::Vector => (
                if self.use_vector_length {
                    "Offset direction and distance"
                } else {
                    "Offset direction"
                },
                "UserSelectedDirection",
                format!("UseVectorLength={}", yes_no(self.use_vector_length)),
            ),
        };
        format!(
            "{direction_prompt} <{}> (SetBasePoint DirectionMethod={method} {direction_options} DeleteInput={} Solid={} BothSides={} AllowDisjoint={} DynamicPreview={})",
            self.distance,
            yes_no(self.delete_input),
            yes_no(self.solid),
            yes_no(self.both_sides),
            yes_no(self.allow_disjoint),
            yes_no(self.dynamic_preview)
        )
    }
}

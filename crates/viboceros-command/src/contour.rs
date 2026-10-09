//! Equally spaced 3D plane cuts, with exact grid indexing and atomic admission.
use super::*;
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{ToPrimitive, Zero};
use viboceros_geometry::Frame3;
mod brep;
mod geometry;
#[cfg(test)]
mod tests;

pub(super) struct ContourCommand;
pub const CONTOUR_USAGE: &str = "Contour base-point direction-point spacing [Range=Yes|No] [AssignProperties=ByCurrentLayer|ByInputObject] [Output=All|CurvesOnly] [GroupObjectsByContourPlane=Yes|No]";
/// A resource limit rejects an excessive job before any model change.
pub const MAX_CONTOUR_PLANES: usize = 100_000;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContourOptions {
    pub range: bool,
    pub input_properties: bool,
    pub group: bool,
}
pub fn parse_options(words: &[&str]) -> Result<ContourOptions, CommandError> {
    let mut options = ContourOptions::default();
    let mut seen = BTreeSet::new();
    for word in words {
        let (key, value) = word
            .trim_start_matches('_')
            .split_once('=')
            .ok_or(CommandError::Usage(CONTOUR_USAGE))?;
        let key = key.to_ascii_lowercase();
        let value = value.trim_start_matches('_').to_ascii_lowercase();
        if !seen.insert(key.clone()) {
            return Err(CommandError::Usage(CONTOUR_USAGE));
        }
        let boolean = || match value.as_str() {
            "yes" => Ok(true),
            "no" => Ok(false),
            _ => Err(CommandError::Usage(CONTOUR_USAGE)),
        };
        match key.as_str() {
            "range" => options.range = boolean()?,
            "groupobjectsbycontourplane" => options.group = boolean()?,
            "assignproperties" => {
                options.input_properties = match value.as_str() {
                    "bycurrentlayer" => false,
                    "byinputobject" => true,
                    _ => return Err(CommandError::Usage(CONTOUR_USAGE)),
                }
            }
            "output" if matches!(value.as_str(), "all" | "curvesonly") => {}
            _ => return Err(CommandError::Usage(CONTOUR_USAGE)),
        }
    }
    Ok(options)
}
fn exact(value: f64) -> BigRational {
    BigRational::from_float(value).expect("validated finite geometry")
}
fn projected(point: Point3, base: Point3, normal: Vector3) -> BigRational {
    point
        .to_array()
        .into_iter()
        .zip(base.to_array())
        .zip(normal.to_array())
        .fold(BigRational::zero(), |sum, ((p, b), n)| {
            sum + (exact(p) - exact(b)) * exact(n)
        })
}
/// The grid is anchored at the base point, even when that point lies far
/// outside the source bounds. Rational indices avoid accumulated spacing drift
/// and retain grids whose integer index exceeds f64's exact integer range.
fn plane_origins(
    inputs: &[&viboceros_document::Object],
    base: Point3,
    normal: Vector3,
    length: f64,
    spacing: f64,
    range: bool,
    tolerance: Tolerance,
) -> Result<Vec<Point3>, CommandError> {
    if !spacing.is_finite() || spacing <= 0. {
        return Err(CommandError::InvalidNumber(spacing.to_string()));
    }
    let mut low: Option<BigRational> = None;
    let mut high: Option<BigRational> = None;
    for object in inputs {
        let bounds = object.geometry().bounds();
        let a = bounds.min().to_array();
        let b = bounds.max().to_array();
        for mask in 0..8 {
            let point = Point3::try_from(std::array::from_fn::<_, 3, _>(|axis| {
                if mask & (1 << axis) == 0 {
                    a[axis]
                } else {
                    b[axis]
                }
            }))?;
            let p = projected(point, base, normal);
            low = Some(low.map_or_else(|| p.clone(), |v| v.min(p.clone())));
            high = Some(high.map_or_else(|| p.clone(), |v| v.max(p.clone())));
        }
    }
    let padding = exact(tolerance.absolute());
    let mut low = low.ok_or(CommandError::NoObjectsSelected)? - &padding;
    let mut high = high.unwrap() + &padding;
    if range {
        low = low.max(BigRational::zero());
        high = high.min(exact(length) + padding);
    }
    let spacing = exact(spacing);
    let first = (low / &spacing).ceil().to_integer();
    let mut last = (high / &spacing).floor().to_integer();
    if range {
        last = last.min((exact(length) / &spacing).floor().to_integer() - BigInt::from(1));
    }
    if last < first {
        return Ok(Vec::new());
    }
    let count = (&last - &first + BigInt::from(1))
        .to_usize()
        .filter(|n| *n <= MAX_CONTOUR_PLANES)
        .ok_or(CommandError::ContourPlaneLimit)?;
    let base = base.to_array().map(exact);
    let normal = normal.to_array().map(exact);
    let mut points = Vec::with_capacity(count);
    for i in 0..count {
        let offset = BigRational::from_integer(&first + BigInt::from(i)) * &spacing;
        let mut point = [0.; 3];
        for axis in 0..3 {
            point[axis] = (&base[axis] + &normal[axis] * &offset)
                .to_f64()
                .filter(|v| v.is_finite())
                .ok_or(CommandError::ContourUnrepresentablePlane)?;
        }
        let point = Point3::try_from(point)?;
        if points.last() == Some(&point) {
            return Err(CommandError::ContourUnrepresentablePlane);
        }
        points.push(point);
    }
    Ok(points)
}
impl Command for ContourCommand {
    fn name(&self) -> &'static str {
        "Contour"
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let Ok(options) = parse_options(args) else {
            return Ok(None);
        };
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Section,
            options: vec![
                BooleanSelectionOption {
                    name: "Range",
                    value: options.range,
                    aliases: &[],
                },
                BooleanSelectionOption {
                    name: "GroupObjectsByContourPlane",
                    value: options.group,
                    aliases: &[],
                },
            ],
            menus: Vec::new(),
            choices: vec![ChoiceSelectionOption {
                name: "AssignProperties",
                value: if options.input_properties {
                    "ByInputObject"
                } else {
                    "ByCurrentLayer"
                },
                choices: &["ByCurrentLayer", "ByInputObject"],
                toggle: None,
            }],
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
        }))
    }

    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let (base, a) = parse_point(args)?;
        let (end, b) = parse_point(&args[a..])?;
        let rest = &args[a + b..];
        let spacing = parse_finite_real(rest.first().ok_or(CommandError::Usage(CONTOUR_USAGE))?)?;
        let options = parse_options(&rest[1..])?;
        let delta = base.vector_to(end)?;
        let length = delta.length()?;
        let frame = Frame3::try_from_normal(base, delta, doc.tolerance())?;
        let inputs = doc
            .selected_objects()
            .filter(|o| ObjectSelectionFilter::Section.accepts_object(o))
            .collect::<Vec<_>>();
        let planes = plane_origins(
            &inputs,
            base,
            frame.z_axis().as_vector(),
            length,
            spacing,
            options.range,
            doc.tolerance(),
        )?;
        let mut staged = Vec::new();
        for origin in planes {
            let frame = Frame3::try_from_normal(origin, delta, doc.tolerance())?;
            let mut plane = Vec::new();
            for source in inputs.iter().rev() {
                for geometry in geometry::cut(source.geometry(), frame, doc.tolerance())? {
                    let attributes = if options.input_properties {
                        source.attributes().clone()
                    } else {
                        viboceros_document::ObjectAttributes::on_layer(doc.current_layer_id())
                    };
                    plane.push((geometry, attributes));
                }
            }
            staged.push(plane);
        }
        let mut groups = Vec::new();
        for plane in &staged {
            groups.push(if options.group && plane.len() > 1 {
                Some(doc.add_empty_group(Some(doc.next_unused_group_name()))?)
            } else {
                None
            });
        }
        let mut outputs = Vec::new();
        for (plane, group) in staged.into_iter().zip(groups).rev() {
            let mut ids = Vec::new();
            for (geometry, attributes) in plane {
                ids.push(doc.add_geometry_with_attributes(geometry, attributes)?);
            }
            if let Some(group) = group {
                doc.add_group_members(group, ids.iter().copied())?;
            }
            outputs.extend(ids);
        }
        let count = outputs.len();
        doc.select_command_results(outputs)?;
        Ok(format!("Created {count} contour objects"))
    }
}

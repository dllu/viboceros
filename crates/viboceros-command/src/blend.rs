//! Creates an independent blend between two selected curve ends.

use super::*;
use viboceros_geometry::{CurveBlendContinuity, CurveBlendOptions, try_blend_curve};

const USAGE: &str = "Blend [Pick1=x,y,z] [Pick2=x,y,z] [Continuity1=Position|Tangency|Curvature] [Continuity2=Position|Tangency|Curvature] [Handle1=length] [Handle2=length]";

pub(super) struct BlendCurveCommand;

struct BlendOptions {
    picks: [Option<Point3>; 2],
    continuity: [CurveBlendContinuity; 2],
    handles: [Option<Real>; 2],
}

impl Command for BlendCurveCommand {
    fn name(&self) -> &'static str {
        "Blend"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse(arguments)?;
        let selected = document
            .selected_objects()
            .map(|object| {
                object
                    .geometry()
                    .curve_ref()
                    .map(|curve| curve.to_owned())
                    .ok_or(CommandError::BlendRequiresTwoCurves)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let [first, second] = selected.as_slice() else {
            return Err(CommandError::BlendRequiresTwoCurves);
        };
        let (default_first, default_second) = super::fillet::nearest_ends(first, second)?;
        let blend = try_blend_curve(
            first,
            options.picks[0].unwrap_or(default_first),
            second,
            options.picks[1].unwrap_or(default_second),
            CurveBlendOptions {
                continuity: options.continuity,
                handles: options.handles,
            },
            document.tolerance(),
        )?;
        let id = document.add_geometry(Geometry::NurbsCurve(blend))?;
        document.select_objects_direct([id], SelectionMode::Replace)?;
        Ok("Created a curve blend".to_string())
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        parse(arguments)?;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Curves,
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
            menus: vec![],
            choices: vec![],
            options: vec![],
        }))
    }
}

fn parse(arguments: &[&str]) -> Result<BlendOptions, CommandError> {
    let mut picks = [None, None];
    let mut continuity = [None, None];
    let mut handles = [None, None];
    for argument in arguments {
        let (name, value) = argument.split_once('=').ok_or(CommandError::Usage(USAGE))?;
        let index = if name.ends_with('1') {
            0
        } else if name.ends_with('2') {
            1
        } else {
            return Err(CommandError::Usage(USAGE));
        };
        let stem = &name[..name.len() - 1];
        if option_name_eq(stem, "Pick") {
            if picks[index].is_some() {
                return Err(CommandError::Usage(USAGE));
            }
            let (point, consumed) = parse_point(&[value])?;
            if consumed != 1 {
                return Err(CommandError::Usage(USAGE));
            }
            picks[index] = Some(point);
        } else if option_name_eq(stem, "Continuity") {
            let mode = if value.eq_ignore_ascii_case("Position") {
                CurveBlendContinuity::Position
            } else if value.eq_ignore_ascii_case("Tangency") {
                CurveBlendContinuity::Tangency
            } else if value.eq_ignore_ascii_case("Curvature") {
                CurveBlendContinuity::Curvature
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            if continuity[index].replace(mode).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
        } else if option_name_eq(stem, "Handle") {
            let handle = parse_finite_real(value)?;
            if handle <= 0.0 || handles[index].replace(handle).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
        } else {
            return Err(CommandError::Usage(USAGE));
        }
    }
    Ok(BlendOptions {
        picks,
        continuity: continuity.map(|value| value.unwrap_or(CurveBlendContinuity::Tangency)),
        handles,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_creates_one_selected_nurbs_and_preserves_sources_through_undo() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Line 0,0,0 1,0,0").unwrap();
        registry.execute(&mut document, "Line 4,1,0 4,2,0").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        registry
            .execute(
                &mut document,
                "Blend Continuity1=Tangency Continuity2=Tangency Handle1=0.5 Handle2=0.75",
            )
            .unwrap();
        assert_eq!(document.objects().count(), 3);
        let blend = document
            .selected_objects()
            .find_map(|object| match object.geometry() {
                Geometry::NurbsCurve(curve) => Some(curve),
                _ => None,
            })
            .expect("selected blend curve");
        assert_eq!(blend.degree(), 3);
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    }

    #[test]
    fn invalid_options_do_not_edit_the_document() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Line 0,0,0 1,0,0").unwrap();
        registry.execute(&mut document, "Line 4,1,0 4,2,0").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        assert!(registry.execute(&mut document, "Blend Handle1=0").is_err());
        assert!(
            registry
                .execute(&mut document, "Blend Continuity2=G3")
                .is_err()
        );
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    }

    #[test]
    fn curvature_option_creates_quartic_without_changing_sources() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Line 0,0,0 1,0,0").unwrap();
        registry.execute(&mut document, "Line 4,1,0 4,2,0").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute(&mut document, "Blend Continuity1=Curvature")
            .unwrap();
        assert_eq!(document.objects().count(), 3);
        let blend = document
            .selected_objects()
            .find_map(|object| match object.geometry() {
                Geometry::NurbsCurve(curve) => Some(curve),
                _ => None,
            })
            .expect("selected quartic blend");
        assert_eq!(blend.degree(), 4);
        let controls = blend.control_points();
        assert!((controls[1].point().x() - 2.1717082451262844).abs() < 1e-12);
        assert!((controls[3].point().y() - 0.060747286907391285).abs() < 1e-12);
    }

    #[test]
    fn mixed_parallel_default_uses_the_rhino_control_position() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Line 0,0,0 1,0,0").unwrap();
        registry.execute(&mut document, "Line 4,0,0 5,0,0").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute(&mut document, "Blend Continuity2=Position")
            .unwrap();
        let blend = document
            .selected_objects()
            .find_map(|object| match object.geometry() {
                Geometry::NurbsCurve(curve) => Some(curve),
                _ => None,
            })
            .expect("selected quadratic blend");
        assert_eq!(blend.degree(), 2);
        let control = blend.control_points()[1].point().to_array();
        assert!((control[0] - 2.2).abs() < 1e-12);
        assert_eq!([control[1], control[2]], [0.0, 0.0]);
    }
}

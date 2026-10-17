//! Arc adds a contact-interval choice to the shared tangent construction getter.
use crate::{CommandError, circle_around_input::Preferences, circle_tangent_input::Input};
use viboceros_document::Document;
use viboceros_geometry::{Circle3, CircularArc3, Frame3, GeometryError, Point3, Real, Tolerance};

/// A pair of straight tangent guides supplies ordered arc endpoints; the
/// remaining curve or point determines the supporting circle. Other triples expose each
/// contact sector, as do two-contact constructions.
#[derive(Clone, Debug)]
pub struct Support {
    pub circle: Circle3,
    pub contact_points: Vec<Point3>,
    pub endpoint_pair: Option<[usize; 2]>,
}
impl Support {
    pub fn from_contacts(input: &Input) -> Result<Self, CommandError> {
        let fit = input.solved_contacts()?;
        Ok(Self {
            circle: fit.circle,
            contact_points: fit.contact_points,
            endpoint_pair: input.arc_line_pair()?,
        })
    }
    pub fn choose(
        &self,
        choice: Point3,
        tolerance: Tolerance,
    ) -> Result<CircularArc3, GeometryError> {
        let Some([first, second]) = self.endpoint_pair else {
            return viboceros_geometry::try_tangent_arc(
                self.circle,
                &self.contact_points,
                choice,
                tolerance,
            );
        };
        let endpoint = |i| {
            self.contact_points
                .get(i)
                .copied()
                .ok_or(GeometryError::Degenerate {
                    context: "missing tangent arc endpoint",
                })
        };
        let a = endpoint(first)?;
        let b = endpoint(second)?;
        let arc = viboceros_geometry::try_tangent_arc(self.circle, &[a, b], choice, tolerance)?;
        if arc.start()?.distance_to(a)? > arc.end()?.distance_to(a)? {
            let length = arc.radius() * arc.sweep_radians();
            arc.reversed(tolerance)?.try_reparameterized(0.0..=length)
        } else {
            Ok(arc)
        }
    }
}

pub struct Definition {
    pub arc: CircularArc3,
}

pub fn from_arguments(
    doc: &Document,
    args: &[&str],
    plane: Frame3,
    radius: Real,
) -> Result<Definition, CommandError> {
    let mut input = Input::new(
        plane,
        doc.tolerance(),
        Preferences {
            radius,
            ..Default::default()
        },
    );
    let mut used = 0;
    while used < args.len() {
        let (count, result) = input.argument(doc, &args[used..])?;
        used += count;
        if result.is_some() {
            let (choice, count) = crate::parse_point(&args[used..])?;
            crate::require_consumed(
                args,
                used + count,
                "Arc Tangent contacts [Enter] choice-point",
            )?;
            let support = Support::from_contacts(&input)?;
            return Ok(Definition {
                arc: support.choose(choice, doc.tolerance())?,
            });
        }
    }
    Err(CommandError::Usage(
        "Arc Tangent requires contacts and a final arc choice",
    ))
}

pub fn command(contacts: &Input, choice: Point3) -> String {
    let [x, y, z] = choice.to_array();
    format!(
        "Arc {} {x},{y},{z}",
        contacts
            .command()
            .strip_prefix("Circle ")
            .expect("shared tangent contact command")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CommandRegistry;
    #[test]
    fn arc_tangent_requires_a_valid_final_choice_and_keeps_failed_commands_atomic() {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        let before = format!("{doc:?}");
        for command in [
            "Arc Tangent Point 3,0,0 Point 0,4,0 Enter",
            "Arc Tangent Point 3,0,0 Point 0,4,0 Enter 1.5,2,0",
            "Arc Tangent Point 3,0,0 0 Point 0,4,0 0,0,0",
            "Arc Tangent Point 3,0,0 Point 0,4,0 Enter 0,0,0 trailing",
        ] {
            assert!(registry.execute(&mut doc, command).is_err(), "{command}");
            assert_eq!(format!("{doc:?}"), before);
            assert_eq!(registry.arc_tangent_radius(), 1.);
            assert!(doc.undo().unwrap().is_none());
        }
        registry
            .execute(&mut doc, "Arc Tangent Point 3,0,0 3 Point 0,4,0 0,0,0")
            .unwrap();
        let saved = registry.arc_tangent_radius();
        assert!((saved - 3.).abs() < 1e-12);
        doc.undo().unwrap();
        assert_eq!(registry.arc_tangent_radius(), saved);
        doc.redo().unwrap();
        registry
            .execute(&mut doc, "Arc Center 0,0,0 2,0,0 90")
            .unwrap();
        assert_eq!(registry.arc_tangent_radius(), saved);
        assert_eq!(CommandRegistry::with_builtins().arc_tangent_radius(), 1.);
    }
}

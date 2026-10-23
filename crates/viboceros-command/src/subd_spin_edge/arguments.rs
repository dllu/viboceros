//! Explicit script targets and local command actions share the interactive session.
use super::*;
pub const USAGE: &str = "SubDSpinEdge [Direction=Clockwise|CounterClockwise] (point|Edge=index) [SpinEdgeAgain|Undo ...]";
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Direction {
    Clockwise,
    #[default]
    CounterClockwise,
}
impl Direction {
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim_start_matches('_');
        if value.eq_ignore_ascii_case("Clockwise") {
            Some(Self::Clockwise)
        } else if value.eq_ignore_ascii_case("CounterClockwise")
            || value.eq_ignore_ascii_case("Counter-clockwise")
        {
            Some(Self::CounterClockwise)
        } else {
            None
        }
    }
}
pub(super) fn run(
    document: &mut Document,
    args: &[&str],
) -> Result<(String, Direction), CommandError> {
    let sources = document.selected_objects().collect::<Vec<_>>();
    if sources.is_empty() {
        return Err(CommandError::NoObjectsSelected);
    }
    if sources
        .iter()
        .any(|o| !matches!(o.geometry(), Geometry::SubD(_)))
    {
        return Err(CommandError::SpinEdgeUnavailable);
    }
    let mut session = SpinSession::new(document, Direction::default());
    let mut i = 0;
    let mut picked = false;
    while i < args.len() {
        let token = args[i].trim_start_matches('_');
        if token.eq_ignore_ascii_case("SpinEdgeAgain") {
            session.spin_again(document)?;
            i += 1;
            continue;
        }
        if token.eq_ignore_ascii_case("Undo") {
            session.undo_spin();
            i += 1;
            continue;
        }
        if token.eq_ignore_ascii_case("Enter") || token.eq_ignore_ascii_case("Cancel") {
            if i + 1 != args.len() {
                return Err(CommandError::Usage(USAGE));
            }
            break;
        }
        let option = if let Some((key, value)) = token.split_once('=') {
            Some((key, value, 1))
        } else if token.eq_ignore_ascii_case("Edge") {
            Some((
                token,
                *args.get(i + 1).ok_or(CommandError::Usage(USAGE))?,
                2,
            ))
        } else if token.eq_ignore_ascii_case("Direction") {
            if let Some(direction) = args.get(i + 1).and_then(|s| Direction::parse(s)) {
                session.direction = direction;
                i += 2;
            } else {
                session.direction = if session.direction == Direction::Clockwise {
                    Direction::CounterClockwise
                } else {
                    Direction::Clockwise
                };
                i += 1;
            }
            continue;
        } else {
            None
        };
        let target = if let Some((key, value, used)) = option {
            if key.eq_ignore_ascii_case("Direction") {
                session.direction = Direction::parse(value).ok_or(CommandError::Usage(USAGE))?;
                i += used;
                continue;
            }
            if !key.eq_ignore_ascii_case("Edge") || sources.len() != 1 {
                return Err(CommandError::Usage(USAGE));
            }
            i += used;
            Target {
                object: sources[0].id(),
                edge: value
                    .parse::<usize>()
                    .map_err(|_| CommandError::Usage(USAGE))?,
            }
        } else {
            let (point, used) = parse_point(&args[i..])?;
            i += used;
            let mut best = None;
            for object in &sources {
                let Geometry::SubD(original) = object.geometry() else {
                    unreachable!()
                };
                let subd = session
                    .sources
                    .get(&object.id())
                    .map_or(original, |s| &s.candidate);
                let polylines = if document.grip_mode(object.id()).is_some() {
                    subd.edges()
                        .iter()
                        .map(|&[a, b]| {
                            vec![
                                subd.control_points()[a as usize],
                                subd.control_points()[b as usize],
                            ]
                        })
                        .collect()
                } else {
                    subd.display_edge_polylines()?
                };
                for (edge, points) in polylines.iter().enumerate() {
                    for pair in points.windows(2) {
                        let closest = if pair[0].is_near(pair[1], document.tolerance()) {
                            pair[0]
                        } else {
                            LineSegment::try_new(pair[0], pair[1], document.tolerance())?
                                .closest_point(point, document.tolerance())?
                        };
                        let distance = point.distance_to(closest)?;
                        if best.is_none_or(|(d, _)| distance < d) {
                            best = Some((
                                distance,
                                Target {
                                    object: object.id(),
                                    edge,
                                },
                            ));
                        }
                    }
                }
            }
            best.ok_or(CommandError::SpinEdgeUnavailable)?.1
        };
        session.spin(document, target)?;
        picked = true;
    }
    if !picked {
        return Err(CommandError::Usage(USAGE));
    }
    let direction = session.direction;
    session.apply(document).map(|o| (o.message, direction))
}

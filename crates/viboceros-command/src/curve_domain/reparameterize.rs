//! Curve and trimmed-face parameter charts, retaining document identity.
use super::*;
use std::borrow::Cow;

#[cfg(test)]
mod tests;

pub(crate) const REPARAMETERIZE_USAGE: &str = "Reparameterize [Object=id | Surface=id] [Member=path/to/leaf] [Face=index] Automatic | Reparameterize [Object=id] [Member=path/to/leaf] start end | Reparameterize [Object=id | Surface=id] [Member=path/to/leaf] [Face=index] u_start u_end v_start v_end";

#[derive(Clone, Debug, PartialEq)]
pub enum ReparameterizeDomains {
    Automatic,
    Explicit(Vec<Real>),
}

struct ReparameterizeInput {
    options: ReparameterizeDomains,
    source: Option<ObjectId>,
    face: Option<usize>,
    member: Option<Vec<usize>>,
    surface_only: bool,
}

enum ReparameterizeTarget {
    Curve(Curve3),
    Surface(NurbsSurface),
    Face(Brep, usize),
}

/// An object or an immutable placed block leaf, optionally restricted to a face.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ReparameterizeReference {
    pub object: ObjectId,
    pub member: Option<Vec<usize>>,
    pub face: Option<usize>,
}

#[derive(Clone, Debug)]
struct Plan {
    reference: ReparameterizeReference,
    source: viboceros_document::Object,
    geometry: Geometry,
    text: BTreeMap<String, String>,
    description: String,
}

/// Stages all parameter charts before making one undoable document edit.
#[derive(Clone, Debug)]
pub struct ReparameterizeSelection {
    plans: Vec<Plan>,
    tolerance: Tolerance,
    output_layer: Option<viboceros_document::LayerId>,
}
impl ReparameterizeSelection {
    pub fn prepare(
        document: &Document,
        references: impl IntoIterator<Item = ReparameterizeReference>,
        domains: ReparameterizeDomains,
    ) -> Result<Self, CommandError> {
        let mut plans = Vec::new();
        let mut seen = BTreeSet::new();
        let mut updated = BTreeMap::<ObjectId, Geometry>::new();
        let mut curves = None;
        let tolerance = document.tolerance();
        for reference in references {
            if !seen.insert(reference.clone()) {
                continue;
            }
            if seen.len() > MAX_SPAN_OUTPUT_OBJECTS {
                return Err(too_many_span_outputs("Reparameterize"));
            }
            let object = document
                .object(reference.object)
                .filter(|_| document.is_object_selectable(reference.object))
                .ok_or(CommandError::Usage(REPARAMETERIZE_USAGE))?;
            let (geometry, text) = if let Some(path) = &reference.member {
                let Geometry::BlockInstance(instance) = object.geometry() else {
                    return Err(CommandError::Usage(REPARAMETERIZE_USAGE));
                };
                let leaf = instance
                    .members()
                    .iter()
                    .find(|leaf| {
                        leaf.path
                            .iter()
                            .map(|p| p.member_index)
                            .eq(path.iter().copied())
                    })
                    .ok_or(CommandError::Usage(REPARAMETERIZE_USAGE))?;
                let display = document.block_member_display(object.attributes(), &leaf.path)?;
                if !display.visible || display.locked {
                    return Err(CommandError::Usage(REPARAMETERIZE_USAGE));
                }
                let copy = if let Geometry::Brep(brep) = &*leaf.geometry {
                    if placed_member_transform(document, instance, leaf)?.orientation_reversing()? {
                        // ON_Brep::Transform retains prototype face flags. The
                        // display cache instead preserves the material side.
                        Some(Geometry::Brep(brep.reversed()))
                    } else {
                        None
                    }
                } else {
                    analytic_block_curve(document, instance, leaf, tolerance)?
                };
                let geometry = copy.map_or_else(|| Cow::Borrowed(&*leaf.geometry), Cow::Owned);
                (geometry, leaf.geometry_user_text.clone())
            } else {
                (
                    Cow::Borrowed(updated.get(&reference.object).unwrap_or(object.geometry())),
                    object.geometry_user_text().clone(),
                )
            };
            let target = target_from_geometry(&geometry, reference.face)?;
            let curve = matches!(&target, ReparameterizeTarget::Curve(_));
            if curves.is_some_and(|previous| previous != curve) {
                return Err(CommandError::ReparameterizeMixedGeometry);
            }
            curves = Some(curve);
            let (geometry, description) =
                reparameterized_target(target, domains.clone(), tolerance)?;
            if reference.member.is_none() {
                updated.insert(reference.object, geometry.clone());
            }
            plans.push(Plan {
                reference,
                source: object.clone(),
                geometry,
                text,
                description,
            });
        }
        if plans.is_empty() {
            return Err(CommandError::ReparameterizeRequiresOneObject { actual: 0 });
        }
        let output_layer = plans
            .iter()
            .any(|p| p.reference.member.is_some())
            .then_some(document.current_layer_id());
        Ok(Self {
            plans,
            tolerance,
            output_layer,
        })
    }
    pub fn commit(&self, document: &mut Document) -> Result<String, CommandError> {
        run_command_transaction(document, "Reparameterize", |doc| self.apply(doc))
    }
    fn apply(&self, document: &mut Document) -> Result<String, CommandError> {
        if document.tolerance() != self.tolerance
            || self
                .output_layer
                .is_some_and(|layer| layer != document.current_layer_id())
            || self.plans.iter().any(|plan| {
                !document.is_object_selectable(plan.reference.object)
                    || document.object(plan.reference.object) != Some(&plan.source)
            })
        {
            return Err(CommandError::Usage(REPARAMETERIZE_USAGE));
        }
        for plan in &self.plans {
            if let Some(path) = &plan.reference.member {
                let Geometry::BlockInstance(instance) = plan.source.geometry() else {
                    unreachable!()
                };
                let leaf = instance
                    .members()
                    .iter()
                    .find(|leaf| {
                        leaf.path
                            .iter()
                            .map(|p| p.member_index)
                            .eq(path.iter().copied())
                    })
                    .ok_or(CommandError::Usage(REPARAMETERIZE_USAGE))?;
                let display =
                    document.block_member_display(plan.source.attributes(), &leaf.path)?;
                if !display.visible || display.locked {
                    return Err(CommandError::Usage(REPARAMETERIZE_USAGE));
                }
            }
        }
        // All plans are validated before publication. Repeated faces on a bare
        // Brep accumulate in place; a placed leaf always produces its own copy.
        let mut updated = BTreeMap::new();
        let mut copies = 0;
        for plan in &self.plans {
            if plan.reference.member.is_some() {
                let id = document.add_geometry(plan.geometry.clone())?;
                for (key, value) in &plan.text {
                    document.set_object_geometry_user_text([id], key, Some(value))?;
                }
                copies += 1;
            } else {
                updated.insert(plan.reference.object, plan);
            }
        }
        for (id, plan) in updated {
            document.replace_object_geometries([(id, plan.geometry.clone())])?;
            for (key, value) in &plan.text {
                document.set_object_geometry_user_text([id], key, Some(value))?;
            }
        }
        if self.plans.len() == 1 && copies == 0 {
            return Ok(format!(
                "Reparameterized the selected {}",
                self.plans[0].description
            ));
        }
        Ok(format!(
            "Reparameterized {} curve(s) or surface(s); added {copies} block copy/copies",
            self.plans.len()
        ))
    }
}

fn analytic_block_curve(
    document: &Document,
    instance: &viboceros_document::BlockInstance,
    leaf: &viboceros_document::ResolvedBlockMember,
    tolerance: Tolerance,
) -> Result<Option<Geometry>, CommandError> {
    // Native parameter editing retains an analytic ArcCurve even when its
    // placement is nonuniform. Keep that public OpenNURBS approximation local
    // to this command; the immutable display cache retains the exact ellipse.
    if !matches!(&*leaf.geometry, Geometry::NurbsCurve(_)) {
        return Ok(None);
    }
    let last = leaf
        .path
        .last()
        .ok_or(CommandError::Usage(REPARAMETERIZE_USAGE))?;
    let definition = document
        .block_definition(last.definition)
        .ok_or(CommandError::Usage(REPARAMETERIZE_USAGE))?;
    let Some(viboceros_document::BlockContent::Geometry(source)) = definition
        .members()
        .get(last.member_index)
        .map(|m| m.content())
    else {
        return Ok(None);
    };
    let circle = match &**source {
        Geometry::Circle(circle) => *circle,
        Geometry::Arc(arc) => viboceros_geometry::Circle3::try_from_frame(
            arc.center(),
            arc.radius(),
            arc.plane_x_axis(),
            arc.normal()?,
            tolerance,
        )?,
        _ => return Ok(None),
    };
    let transform = placed_member_transform(document, instance, leaf)?;
    let circle = circle.transformed_as_circle(transform, tolerance)?;
    Ok(Some(match &**source {
        Geometry::Circle(_) => Geometry::Circle(circle),
        Geometry::Arc(arc) => Geometry::Arc(
            viboceros_geometry::CircularArc3::try_from_circle_angles(circle, arc.angle_domain())?
                .try_reparameterized(arc.domain())?,
        ),
        _ => unreachable!(),
    }))
}

fn placed_member_transform(
    document: &Document,
    instance: &viboceros_document::BlockInstance,
    leaf: &viboceros_document::ResolvedBlockMember,
) -> Result<viboceros_geometry::AffineTransform3, CommandError> {
    let mut transform = instance.reference().transform();
    for location in leaf.path.iter().take(leaf.path.len() - 1) {
        let definition = document
            .block_definition(location.definition)
            .ok_or(CommandError::Usage(REPARAMETERIZE_USAGE))?;
        let Some(viboceros_document::BlockContent::Reference(reference)) = definition
            .members()
            .get(location.member_index)
            .map(|m| m.content())
        else {
            return Err(CommandError::Usage(REPARAMETERIZE_USAGE));
        };
        transform = reference.transform().then(transform)?;
    }
    Ok(transform)
}

fn target_from_geometry(
    geometry: &Geometry,
    face: Option<usize>,
) -> Result<ReparameterizeTarget, CommandError> {
    match geometry {
        Geometry::NurbsSurface(surface) if face.is_none_or(|i| i == 0) => {
            Ok(ReparameterizeTarget::Surface(surface.clone()))
        }
        Geometry::Brep(brep) => {
            let index = face
                .or_else(|| (brep.faces().len() == 1).then_some(0))
                .filter(|&i| i < brep.faces().len())
                .ok_or(CommandError::Usage(REPARAMETERIZE_USAGE))?;
            Ok(ReparameterizeTarget::Face(brep.clone(), index))
        }
        geometry if face.is_none() => geometry
            .curve_ref()
            .map(|c| ReparameterizeTarget::Curve(c.to_owned()))
            .ok_or(CommandError::Usage(REPARAMETERIZE_USAGE)),
        _ => Err(CommandError::Usage(REPARAMETERIZE_USAGE)),
    }
}

pub(crate) struct ReparameterizeCommand;
impl Command for ReparameterizeCommand {
    fn name(&self) -> &'static str {
        "Reparameterize"
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let ReparameterizeInput {
            options,
            source,
            face,
            member,
            surface_only,
        } = parse_arguments(arguments)?;
        let objects = if let Some(id) = source {
            vec![
                document
                    .object(id)
                    .filter(|_| document.is_object_selectable(id))
                    .ok_or(CommandError::Usage(REPARAMETERIZE_USAGE))?,
            ]
        } else {
            document.selected_objects().collect()
        };
        let references = objects
            .into_iter()
            .filter(|o| {
                member.is_some()
                    || matches!(o.geometry(), Geometry::NurbsSurface(_) | Geometry::Brep(_))
                    || (!surface_only && o.geometry().curve_ref().is_some())
            })
            .map(|o| ReparameterizeReference {
                object: o.id(),
                member: member.clone(),
                face,
            })
            .collect::<Vec<_>>();
        if surface_only
            && references.iter().any(|r| {
                document
                    .object(r.object)
                    .is_some_and(|o| o.geometry().curve_ref().is_some())
            })
        {
            return Err(CommandError::Usage(REPARAMETERIZE_USAGE));
        }
        let selection = ReparameterizeSelection::prepare(document, references, options)?;
        if surface_only
            && selection
                .plans
                .iter()
                .any(|p| p.geometry.curve_ref().is_some())
        {
            return Err(CommandError::Usage(REPARAMETERIZE_USAGE));
        }
        selection.apply(document)
    }
}

fn reparameterized_target(
    target: ReparameterizeTarget,
    options: ReparameterizeDomains,
    tolerance: Tolerance,
) -> Result<(Geometry, String), CommandError> {
    Ok(match (target, options) {
        (ReparameterizeTarget::Curve(curve), ReparameterizeDomains::Automatic) => {
            let domain_end = curve.as_ref().length(tolerance)?;
            let curve = reparameterize_native_curve(&curve, 0.0..=domain_end, tolerance)?;
            (
                Geometry::from(curve),
                format!("curve domain to 0..{domain_end}"),
            )
        }
        (ReparameterizeTarget::Curve(curve), ReparameterizeDomains::Explicit(values))
            if values.len() == 2 =>
        {
            let [start, end] = [values[0], values[1]];
            let curve = reparameterize_native_curve(&curve, start..=end, tolerance)?;
            (
                Geometry::from(curve),
                format!("curve domain to {start}..{end}"),
            )
        }
        (ReparameterizeTarget::Surface(surface), ReparameterizeDomains::Automatic) => {
            let [width, height] = automatic_surface_size(&surface, tolerance)?;
            let surface = surface.try_reparameterized(0.0..=width, 0.0..=height)?;
            (
                Geometry::NurbsSurface(surface),
                format!("surface domains to U 0..{width}, V 0..{height}"),
            )
        }
        (ReparameterizeTarget::Surface(surface), ReparameterizeDomains::Explicit(values))
            if values.len() == 4 =>
        {
            let [u_start, u_end, v_start, v_end] = [values[0], values[1], values[2], values[3]];
            let surface = surface.try_reparameterized(u_start..=u_end, v_start..=v_end)?;
            (
                Geometry::NurbsSurface(surface),
                format!("surface domains to U {u_start}..{u_end}, V {v_start}..{v_end}"),
            )
        }
        (ReparameterizeTarget::Face(brep, index), options) => {
            let values = match options {
                ReparameterizeDomains::Automatic => {
                    let [width, height] =
                        automatic_surface_size(brep.faces()[index].surface(), tolerance)?;
                    [0., width, 0., height]
                }
                ReparameterizeDomains::Explicit(v) if v.len() == 4 => [v[0], v[1], v[2], v[3]],
                _ => return Err(CommandError::Usage(REPARAMETERIZE_USAGE)),
            };
            let [u_start, u_end, v_start, v_end] = values;
            let brep = brep.try_reparameterized_face_uv(
                [(index, [u_start..=u_end, v_start..=v_end])],
                tolerance,
            )?;
            (
                Geometry::Brep(brep),
                format!("surface domains to U {u_start}..{u_end}, V {v_start}..{v_end}"),
            )
        }
        _ => return Err(CommandError::Usage(REPARAMETERIZE_USAGE)),
    })
}

fn automatic_surface_size(
    surface: &NurbsSurface,
    tolerance: Tolerance,
) -> Result<[Real; 2], GeometryError> {
    use viboceros_geometry::{ArcLengthSampler, CurveRef, FiniteSum, SurfaceExtensionEdge as Edge};
    let mut lengths = [[0.; 11]; 2];
    let [u_lengths, v_lengths] = &mut lengths;
    for (i, (u_length, v_length)) in u_lengths.iter_mut().zip(v_lengths).enumerate() {
        let fraction = i as Real / 10.;
        let curves = [
            surface.isocurve_u(surface.parameter_at_v(fraction)?)?,
            surface.isocurve_v(surface.parameter_at_u(fraction)?)?,
        ];
        for (axis, (curve, length)) in curves.iter().zip([u_length, v_length]).enumerate() {
            let boundary = match (axis, i) {
                (0, 0) => Some(Edge::South),
                (0, 10) => Some(Edge::North),
                (1, 0) => Some(Edge::West),
                (1, 10) => Some(Edge::East),
                _ => None,
            };
            if boundary.is_some_and(|edge| surface.has_coincident_boundary_controls(edge)) {
                continue;
            }
            // Match the command's default length query through the existing
            // bounded Cash–Karp trace. The general precise length API and
            // caller's geometry acceptance tolerance remain independent.
            *length = match ArcLengthSampler::try_new(CurveRef::NurbsCurve(curve), tolerance) {
                Ok(sampler) => sampler.quadrature_length_estimate(1e-8)?,
                Err(GeometryError::Degenerate {
                    context: "arc-length curve",
                }) if curve.length(tolerance)? == 0. => 0.,
                Err(error) => return Err(error),
            };
        }
    }
    let mut size = [0.; 2];
    for (axis, lengths) in lengths.iter().enumerate() {
        let maximum = lengths.iter().copied().fold(0_f64, Real::max);
        if maximum > 0. {
            let mut sum = FiniteSum::default();
            let mut count = 0;
            for length in lengths.iter().filter(|&&length| length > 0.) {
                sum.add(length / maximum)?;
                count += 1;
            }
            size[axis] = (sum.total()? / count as Real) * maximum;
        }
    }
    Ok(size)
}

fn parse_arguments(arguments: &[&str]) -> Result<ReparameterizeInput, CommandError> {
    let mut source = None;
    let mut face = None;
    let mut member = None;
    let mut surface_only = false;
    let mut values = Vec::new();
    for argument in arguments {
        match argument.split_once('=') {
            Some((name, value))
                if (option_name_eq(name, "Surface") || option_name_eq(name, "Object"))
                    && source.is_none() =>
            {
                surface_only = option_name_eq(name, "Surface");
                source = Some(
                    value
                        .parse::<ObjectId>()
                        .map_err(|_| CommandError::Usage(REPARAMETERIZE_USAGE))?,
                );
            }
            Some((name, value)) if option_name_eq(name, "Face") && face.is_none() => {
                face = Some(
                    value
                        .parse::<usize>()
                        .map_err(|_| CommandError::Usage(REPARAMETERIZE_USAGE))?,
                );
            }
            Some((name, value)) if option_name_eq(name, "Member") && member.is_none() => {
                let path = value
                    .split('/')
                    .map(str::parse::<usize>)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| CommandError::Usage(REPARAMETERIZE_USAGE))?;
                if path.is_empty() || path.len() > 64 {
                    return Err(CommandError::Usage(REPARAMETERIZE_USAGE));
                }
                member = Some(path);
            }
            _ => values.push(*argument),
        }
    }
    if member.is_some() && source.is_none() {
        return Err(CommandError::Usage(REPARAMETERIZE_USAGE));
    }
    Ok(ReparameterizeInput {
        options: parse_reparameterize_options(&values)?,
        source,
        face,
        member,
        surface_only,
    })
}

fn reparameterize_native_curve(
    curve: &Curve3,
    domain: std::ops::RangeInclusive<Real>,
    tolerance: Tolerance,
) -> Result<Curve3, GeometryError> {
    match curve {
        Curve3::PolyCurve(c) => Ok(Curve3::PolyCurve(
            c.try_reparameterized_by_length(domain, tolerance)?,
        )),
        _ => curve.try_reparameterized(domain),
    }
}

fn parse_reparameterize_options(arguments: &[&str]) -> Result<ReparameterizeDomains, CommandError> {
    if let [argument] = arguments {
        let automatic = argument.trim_start_matches(['_', '-']);
        if automatic.eq_ignore_ascii_case("Automatic") {
            return Ok(ReparameterizeDomains::Automatic);
        }
    }
    if arguments.iter().any(|argument| {
        argument
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("Automatic")
    }) {
        return Err(CommandError::Usage(REPARAMETERIZE_USAGE));
    }

    let raw_values = arguments
        .iter()
        .flat_map(|argument| argument.split(','))
        .collect::<Vec<_>>();
    if !matches!(raw_values.len(), 2 | 4) || raw_values.iter().any(|value| value.is_empty()) {
        return Err(CommandError::Usage(REPARAMETERIZE_USAGE));
    }
    Ok(ReparameterizeDomains::Explicit(
        raw_values
            .into_iter()
            .map(parse_finite_real)
            .collect::<Result<_, _>>()?,
    ))
}

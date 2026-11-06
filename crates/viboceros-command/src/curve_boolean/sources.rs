//! Immutable whole-curve and temporary range inputs with distinct edit owners.
use super::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    Whole(ObjectId),
    Subcurve {
        object: ObjectId,
        edge: Option<usize>,
        parameters: [Real; 2],
        wrap_edge: bool,
    },
    Length {
        object: ObjectId,
        parameters: [Real; 2],
    },
}
impl Source {
    pub fn object(&self) -> ObjectId {
        match self {
            Self::Whole(id) => *id,
            Self::Subcurve { object, .. } | Self::Length { object, .. } => *object,
        }
    }
    pub fn parse(value: &str, length: bool) -> Result<Self, CommandError> {
        let words = value.split(',').collect::<Vec<_>>();
        let id = words
            .first()
            .and_then(|s| s.parse().ok())
            .ok_or(CommandError::Usage(USAGE))?;
        match words.as_slice() {
            [_, a, b] => {
                let parameters = [parse_finite_real(a)?, parse_finite_real(b)?];
                Ok(if length {
                    Self::Length {
                        object: id,
                        parameters,
                    }
                } else {
                    Self::Subcurve {
                        object: id,
                        edge: None,
                        parameters,
                        wrap_edge: false,
                    }
                })
            }
            [_, kind, index, a, b] if !length => {
                let wrap_edge = kind.eq_ignore_ascii_case("EdgeWrap");
                if !wrap_edge && !kind.eq_ignore_ascii_case("Edge") {
                    return Err(CommandError::Usage(USAGE));
                }
                Ok(Self::Subcurve {
                    object: id,
                    edge: Some(index.parse().map_err(|_| CommandError::Usage(USAGE))?),
                    parameters: [parse_finite_real(a)?, parse_finite_real(b)?],
                    wrap_edge,
                })
            }
            _ => Err(CommandError::Usage(USAGE)),
        }
    }
    pub fn validate(&self, doc: &Document) -> Result<(), CommandError> {
        self.resolve(doc).map(|_| ())
    }
    fn resolve(&self, doc: &Document) -> Result<Option<NurbsCurve>, CommandError> {
        let object = doc
            .object(self.object())
            .filter(|_| doc.is_object_selectable(self.object()))
            .ok_or(failure("CurveBoolean requires selectable input parents"))?;
        match *self {
            Self::Whole(_) => object
                .geometry()
                .curve_ref()
                .ok_or(failure("CurveBoolean requires curve inputs"))?
                .to_nurbs()
                .map(Some)
                .map_err(Into::into),
            Self::Subcurve {
                object: id,
                edge,
                parameters: [a, b],
                wrap_edge,
            } => {
                let reference = curve_reference::CapturedCurve::capture(doc, id, edge)?;
                let curve = reference.curve_ref();
                let piece = if edge.is_some() && !wrap_edge {
                    curve_reference::edge_subcurve(curve, a, b)?
                } else {
                    prepare_directed_subcurve(&curve.to_owned(), a, b)?
                };
                Ok(Some(piece.as_ref().to_nurbs()?))
            }
            Self::Length {
                parameters: [a, length],
                ..
            } => {
                let curve = object
                    .geometry()
                    .curve_ref()
                    .ok_or(failure("CurveBoolean requires curve inputs"))?;
                if !curve.domain().contains(&a) || !length.is_finite() {
                    return Err(GeometryError::InvalidCurveTrimInterval.into());
                }
                let total = curve.length(doc.tolerance())?;
                if length.abs() > total {
                    return Err(failure("Subcurve length exceeds the whole curve"));
                }
                if length == 0. || (curve.is_closed()? && length.abs() == total) {
                    return Ok(None);
                }
                let Some([a, b]) = subcurve_input::interval_in_direction(
                    curve,
                    a,
                    length.abs(),
                    length >= 0.,
                    doc.tolerance(),
                )?
                else {
                    return Ok(None);
                };
                let piece = prepare_directed_subcurve(&curve.to_owned(), a, b)?;
                Ok(Some(piece.as_ref().to_nurbs()?))
            }
        }
    }
}
pub(super) struct Whole {
    pub parent: usize,
    pub curve: NurbsCurve,
}
pub(super) struct Resolved {
    pub parents: Vec<viboceros_document::Object>,
    pub inputs: Vec<NurbsCurve>,
    pub whole: Vec<(usize, NurbsCurve)>,
}
pub(super) fn resolve(doc: &Document, inputs: &[Source]) -> Result<Resolved, CommandError> {
    if inputs.is_empty() {
        return Err(failure("CurveBoolean requires selected curves"));
    }
    let mut parents = Vec::new();
    let mut parent_index = BTreeMap::new();
    let mut whole = Vec::new();
    let mut full = BTreeSet::new();
    let mut curves = Vec::new();
    let mut controls = 0usize;
    for input in inputs {
        let id = input.object();
        if !doc.is_object_selectable(id) {
            return Err(failure("CurveBoolean requires selectable input parents"));
        }
        let parent = *parent_index.entry(id).or_insert_with(|| {
            let index = parents.len();
            parents.push(doc.object(id).unwrap().clone());
            index
        });
        let Some(curve) = input.resolve(doc)? else {
            continue;
        };
        controls = controls.saturating_add(
            curve
                .spans()
                .count()
                .saturating_mul(curve.degree().saturating_add(1)),
        );
        if controls > 100_000 {
            return Err(GeometryError::CurveJoinLimit {
                resource: "planar network controls",
                maximum: 100_000,
            }
            .into());
        }
        if matches!(input, Source::Whole(_)) {
            if !full.insert(id) {
                return Err(failure("CurveBoolean requires distinct whole curve inputs"));
            }
            whole.push((parent, curve.clone()));
        }
        curves.push(curve);
    }
    Ok(Resolved {
        parents,
        inputs: curves,
        whole,
    })
}

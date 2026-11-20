//! A rail may borrow one curve or own a private assembly of accepted chain pieces.
use super::*;
use crate::surface_edge_reference::Edge;
use crate::{chain_selection::Target, curve_reference::CapturedCurve};
use std::sync::Arc;
use viboceros_geometry::{CurveJoinOptions, CurveJoinStyle, join_curves_with_endpoint_filter};

#[derive(Clone, Debug)]
pub struct Rail {
    pieces: Arc<[CapturedCurve]>,
    supports: Arc<[Edge]>,
    joined: Option<Arc<NurbsCurve>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RailReference {
    pub target: Target,
    pub face: Option<usize>,
}

impl RailReference {
    pub fn parse_edge(value: &str) -> Result<Self, CommandError> {
        let invalid = || {
            CommandError::Usage(
                "Use an edge reference of the form object-id,edge-index[,face-index]",
            )
        };
        let mut parts = value.split(',');
        let object = parts
            .next()
            .and_then(|value| value.parse().ok())
            .ok_or_else(invalid)?;
        let edge = parts
            .next()
            .and_then(|value| value.parse().ok())
            .ok_or_else(invalid)?;
        let face = parts
            .next()
            .map(|value| value.parse().map_err(|_| invalid()))
            .transpose()?;
        if parts.next().is_some() {
            return Err(invalid());
        }
        Ok(Self {
            target: Target::Edge(object, edge),
            face,
        })
    }
}

impl Rail {
    pub fn capture(document: &Document, target: Target) -> Result<Self, CommandError> {
        Self::capture_reference(document, RailReference { target, face: None })
    }

    pub fn capture_reference(
        document: &Document,
        reference: RailReference,
    ) -> Result<Self, CommandError> {
        let support = match reference.target {
            Target::Edge(object, index) if reference.face.is_some() => {
                Some(Edge::capture(document, object, index, reference.face)?)
            }
            Target::Edge(object, index) => Edge::capture(document, object, index, None).ok(),
            Target::Curve(_) => None,
        };
        let piece = match &support {
            Some(support) => support.guide.clone(),
            None => capture_piece(document, reference.target)?,
        };
        Ok(Self {
            pieces: vec![piece].into(),
            supports: support.into_iter().collect::<Vec<_>>().into(),
            joined: None,
        })
    }

    pub fn chain(
        document: &Document,
        path: &[(Target, bool)],
        gap: Real,
    ) -> Result<Self, CommandError> {
        if let [(target, false)] = path {
            return Self::capture(document, *target);
        }
        let tolerance = document.tolerance();
        let pieces = path
            .iter()
            .map(|(target, _)| capture_piece(document, *target))
            .collect::<Result<Vec<_>, _>>()?;
        let curves = pieces
            .iter()
            .zip(path)
            .map(|(piece, (_, reversed))| {
                let curve = piece.curve_ref().to_owned();
                if *reversed {
                    curve.reversed(tolerance)
                } else {
                    Ok(curve)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        // Join only the endpoint pairs chosen by the chain picker. Nearby
        // pieces elsewhere along the path must not replace those connections.
        let joined = join_curves_with_endpoint_filter(
            &curves,
            CurveJoinOptions {
                tolerance: gap,
                preserve_direction: true,
                style: CurveJoinStyle::Seeded,
            },
            tolerance,
            |a, a_start, b, b_start| {
                (a + 1 == b && !a_start && b_start) || (b + 1 == a && !b_start && a_start)
            },
        )?;
        let [rail] = joined.as_slice() else {
            return Err(CommandError::Usage(
                "Select pieces that form one connected sweep rail",
            ));
        };
        let curve = rail.curve().as_ref().to_nurbs()?;
        let supports = path
            .iter()
            .map(|(target, _)| match *target {
                Target::Edge(object, index) => Edge::capture(document, object, index, None).ok(),
                Target::Curve(_) => None,
            })
            .collect::<Option<Vec<_>>>()
            .unwrap_or_default();
        Ok(Self {
            pieces: pieces.into(),
            supports: supports.into(),
            joined: Some(Arc::new(curve)),
        })
    }

    pub fn targets(&self) -> impl Iterator<Item = Target> + '_ {
        self.pieces.iter().map(|piece| {
            let id = piece.object_id();
            piece
                .source()
                .1
                .map_or(Target::Curve(id), |edge| Target::Edge(id, edge))
        })
    }

    pub fn current(&self, document: &Document) -> bool {
        self.pieces.iter().all(|piece| piece.current(document))
    }

    pub fn is_closed(&self) -> bool {
        self.curve().is_closed().is_ok_and(|closed| closed)
    }

    pub(crate) fn supports(&self) -> &[Edge] {
        &self.supports
    }

    pub fn has_surface_support(&self) -> bool {
        !self.supports.is_empty()
    }

    pub(crate) fn curve(&self) -> &NurbsCurve {
        self.joined
            .as_deref()
            .unwrap_or_else(|| self.pieces[0].curve())
    }

    pub(crate) fn curve_ref(&self) -> CurveRef<'_> {
        self.joined
            .as_deref()
            .map_or_else(|| self.pieces[0].curve_ref(), CurveRef::NurbsCurve)
    }

    pub fn supports_alignment(&self) -> bool {
        self.supports.len() == 1
    }
}

fn capture_piece(document: &Document, target: Target) -> Result<CapturedCurve, CommandError> {
    match target {
        Target::Curve(id) => CapturedCurve::capture(document, id, None),
        Target::Edge(id, edge) => CapturedCurve::capture(document, id, Some(edge)),
    }
}

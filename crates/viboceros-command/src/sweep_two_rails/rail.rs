//! A rail may borrow one curve or own a private assembly of accepted chain pieces.
use super::*;
use crate::{chain_selection::Target, curve_reference::CapturedCurve};
use std::sync::Arc;
use viboceros_geometry::{CurveJoinOptions, CurveJoinStyle, join_curves_with_endpoint_filter};

#[derive(Clone, Debug)]
pub struct Rail {
    pieces: Arc<[CapturedCurve]>,
    joined: Option<Arc<NurbsCurve>>,
}

impl Rail {
    pub fn capture(document: &Document, target: Target) -> Result<Self, CommandError> {
        Ok(Self {
            pieces: vec![capture_piece(document, target)?].into(),
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
                "Sweep2: select pieces that form one connected rail",
            ));
        };
        let curve = rail.curve().as_ref().to_nurbs()?;
        Ok(Self {
            pieces: pieces.into(),
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

    pub(super) fn curve(&self) -> &NurbsCurve {
        self.joined
            .as_deref()
            .unwrap_or_else(|| self.pieces[0].curve())
    }
}

fn capture_piece(document: &Document, target: Target) -> Result<CapturedCurve, CommandError> {
    match target {
        Target::Curve(id) => CapturedCurve::capture(document, id, None),
        Target::Edge(id, edge) => CapturedCurve::capture(document, id, Some(edge)),
    }
}

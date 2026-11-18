//! Immutable Pipe sources and normal frames reused by the viewport draft.
use super::*;
use crate::curve_reference::CapturedCurve;
use viboceros_geometry::{Curve3, CurveJoinOptions, CurveJoinStyle, Point3, join_curves};

#[derive(Clone, Debug)]
pub struct Rail {
    sources: Vec<CapturedCurve>,
    owned: Option<Geometry>,
    joined_curve: Option<NurbsCurve>,
    frames: Vec<(f64, Frame3)>,
    closed: bool,
}

impl Rail {
    pub fn capture(
        doc: &Document,
        object: ObjectId,
        edge: Option<usize>,
    ) -> Result<Self, CommandError> {
        let source = CapturedCurve::capture(doc, object, edge)?;
        let owned = edge.map(|_| Geometry::from(source.curve_ref().to_owned()));
        Self::new(vec![source], owned, doc.tolerance())
    }

    pub fn chain(mut rails: Vec<Self>, tolerance: Tolerance) -> Result<Self, CommandError> {
        if rails.len() == 1 {
            return Ok(rails.pop().unwrap());
        }
        let curves = rails
            .iter()
            .map(|rail| rail.curve().to_owned())
            .collect::<Vec<Curve3>>();
        let mut joined = join_curves(
            &curves,
            CurveJoinOptions {
                tolerance: tolerance.absolute(),
                preserve_direction: false,
                style: CurveJoinStyle::Seeded,
            },
            tolerance,
        )?;
        if joined.len() != 1 {
            return Err(CommandError::Usage(
                "Pipe: select edges that form one connected rail",
            ));
        }
        let geometry = Geometry::from(joined.pop().unwrap().into_curve());
        let sources = rails.into_iter().flat_map(|rail| rail.sources).collect();
        Self::new(sources, Some(geometry), tolerance)
    }

    fn new(
        sources: Vec<CapturedCurve>,
        owned: Option<Geometry>,
        tolerance: Tolerance,
    ) -> Result<Self, CommandError> {
        let curve = source_geometry(&sources, owned.as_ref())
            .curve_ref()
            .unwrap();
        let closed = curve.is_closed()?;
        let joined_curve = (sources.len() > 1).then(|| curve.to_nurbs()).transpose()?;
        let mut frames = Vec::<(f64, Frame3)>::with_capacity(33);
        for index in 0..=32 {
            let f = f64::from(index) / 32.;
            let sample = curve.evaluate_with_tangent(curve.parameter_at(f)?);
            let frame = match sample {
                Ok(sample) => {
                    let normal = sample.tangent();
                    frames
                        .last()
                        .and_then(|(_, frame)| {
                            Frame3::try_from_x_and_normal(
                                sample.point(),
                                frame.x_axis().as_vector(),
                                normal.as_vector(),
                                tolerance,
                            )
                            .ok()
                        })
                        .map_or_else(|| Frame3::try_from_unit_normal(sample.point(), normal), Ok)?
                }
                Err(_) if index != 0 && index != 32 => continue,
                Err(error) => return Err(error.into()),
            };
            frames.push((f, frame));
        }
        Ok(Self {
            sources,
            owned,
            joined_curve,
            frames,
            closed,
        })
    }

    pub fn references(&self) -> impl Iterator<Item = (ObjectId, Option<usize>)> + '_ {
        self.sources
            .iter()
            .map(|source| (source.object_id(), source.source().1))
    }

    pub fn current(&self, doc: &Document) -> bool {
        self.sources.iter().all(|source| source.current(doc))
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }

    pub fn projected_curve(&self) -> &NurbsCurve {
        self.joined_curve
            .as_ref()
            .unwrap_or_else(|| self.sources[0].curve())
    }

    pub(super) fn geometry(&self) -> &Geometry {
        source_geometry(&self.sources, self.owned.as_ref())
    }

    pub(super) fn curve(&self) -> CurveRef<'_> {
        self.geometry().curve_ref().unwrap()
    }

    pub(super) fn frames(&self) -> &[(f64, Frame3)] {
        &self.frames
    }

    pub(super) fn frame(&self, fraction: f64) -> Result<Frame3, GeometryError> {
        let curve = self.curve();
        let sample = curve.evaluate_with_tangent(curve.parameter_at(fraction)?)?;
        Frame3::try_from_unit_normal(sample.point(), sample.tangent())
    }

    pub(super) fn fraction_at(
        &self,
        point: Point3,
        tolerance: Tolerance,
    ) -> Result<f64, GeometryError> {
        let curve = self.curve();
        let parameter = curve.closest_parameter(point, tolerance)?;
        let domain = curve.domain();
        Ok(((parameter - domain.start()) / (domain.end() - domain.start())).clamp(0., 1.))
    }
}

fn source_geometry<'a>(sources: &'a [CapturedCurve], owned: Option<&'a Geometry>) -> &'a Geometry {
    owned.unwrap_or_else(|| sources[0].source().0)
}

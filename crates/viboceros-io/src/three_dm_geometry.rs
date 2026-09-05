use viboceros_geometry::{
    Brep, BrepEdge, BrepFace, BrepLoop, BrepLoopType, BrepTrim, BrepTrimType, BrepVertex, Circle3,
    CircularArc3, CurveSegment3, GeometryError, LineSegment, MAX_POLYCURVE_SEGMENTS, NurbsCurve,
    NurbsCurve2, NurbsSurface, Point2, Point3, PolyCurve3, Polyline3, SurfaceIso, Tolerance,
    Vector3, WeightedPoint2, WeightedPoint3,
};

const MAGIC: &[u8; 8] = b"VIBOBRP\0";
const POLYCURVE_MAGIC: &[u8; 8] = b"VIBOPLY\0";
const VERSION: u32 = 1;
const POLYCURVE_VERSION: u32 = 2;
const NO_EDGE: u64 = u64::MAX;

#[derive(Debug)]
pub(crate) enum GeometryCodecError {
    Malformed,
    SizeOverflow,
    Geometry(GeometryError),
}

impl From<GeometryError> for GeometryCodecError {
    fn from(error: GeometryError) -> Self {
        Self::Geometry(error)
    }
}

pub(crate) fn encode_polycurve(curve: &PolyCurve3) -> Result<Vec<u8>, GeometryCodecError> {
    let mut writer = Writer::default();
    writer.bytes.extend_from_slice(POLYCURVE_MAGIC);
    writer.u32(POLYCURVE_VERSION);
    writer.len(curve.segments().len())?;
    for parameter in curve.parameters() {
        writer.f64(*parameter);
    }
    for segment in curve.segments() {
        writer.segment(segment)?;
    }
    Ok(writer.bytes)
}

pub(crate) fn encode_arc(arc: CircularArc3) -> Result<Vec<u8>, GeometryCodecError> {
    let mut writer = Writer::default();
    writer.u32(VERSION);
    writer.segment(&CurveSegment3::Arc(arc))?;
    Ok(writer.bytes)
}

pub(crate) fn decode_arc(bytes: &[u8]) -> Result<CircularArc3, GeometryCodecError> {
    let mut reader = Reader { bytes, position: 0 };
    if reader.u32()? != VERSION {
        return Err(GeometryCodecError::Malformed);
    }
    let CurveSegment3::Arc(arc) = reader.segment()? else {
        return Err(GeometryCodecError::Malformed);
    };
    if reader.position != bytes.len() {
        return Err(GeometryCodecError::Malformed);
    }
    Ok(arc)
}

pub(crate) fn decode_polycurve(bytes: &[u8]) -> Result<PolyCurve3, GeometryCodecError> {
    let mut reader = Reader { bytes, position: 0 };
    if reader.take(POLYCURVE_MAGIC.len())? != POLYCURVE_MAGIC {
        return Err(GeometryCodecError::Malformed);
    }
    let version = reader.u32()?;
    if version != 1 && version != POLYCURVE_VERSION {
        return Err(GeometryCodecError::Malformed);
    }
    let count = reader.len()?;
    if count == 0 || count > MAX_POLYCURVE_SEGMENTS {
        return Err(GeometryCodecError::Malformed);
    }
    let parameters = (0..=count)
        .map(|_| reader.f64())
        .collect::<Result<Vec<_>, _>>()?;
    let segments = (0..count)
        .map(|_| {
            if version == 1 {
                reader.curve3().map(CurveSegment3::NurbsCurve)
            } else {
                reader.segment()
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    if reader.position != bytes.len() {
        return Err(GeometryCodecError::Malformed);
    }
    Ok(PolyCurve3::try_with_segment_domains(segments, parameters)?)
}

pub(crate) fn encode_brep(brep: &Brep) -> Result<Vec<u8>, GeometryCodecError> {
    let mut writer = Writer::default();
    writer.bytes.extend_from_slice(MAGIC);
    writer.u32(VERSION);
    writer.len(brep.vertices().len())?;
    writer.len(brep.edges().len())?;
    writer.len(brep.faces().len())?;

    for vertex in brep.vertices() {
        writer.point3(vertex.point());
        writer.f64(vertex.tolerance());
    }
    for edge in brep.edges() {
        writer.index(edge.vertices()[0])?;
        writer.index(edge.vertices()[1])?;
        writer.f64(edge.tolerance());
        writer.curve3(edge.curve())?;
    }
    for face in brep.faces() {
        writer.boolean(face.is_reversed());
        writer.surface(face.surface())?;
        writer.len(face.loops().len())?;
        for face_loop in face.loops() {
            writer.u8(match face_loop.loop_type() {
                BrepLoopType::Outer => 1,
                BrepLoopType::Inner => 2,
            });
            writer.len(face_loop.trims().len())?;
            for trim in face_loop.trims() {
                writer.index(trim.vertices()[0])?;
                writer.index(trim.vertices()[1])?;
                writer.u64(match trim.edge() {
                    Some(edge) => {
                        u64::try_from(edge).map_err(|_| GeometryCodecError::SizeOverflow)?
                    }
                    None => NO_EDGE,
                });
                writer.boolean(trim.is_reversed_3d());
                writer.u8(match trim.trim_type() {
                    BrepTrimType::Boundary => 1,
                    BrepTrimType::Mated => 2,
                    BrepTrimType::Seam => 3,
                    BrepTrimType::Singular => 4,
                });
                writer.u8(match trim.iso() {
                    SurfaceIso::NotIso => 0,
                    SurfaceIso::InteriorUConstant => 1,
                    SurfaceIso::InteriorVConstant => 2,
                    SurfaceIso::West => 3,
                    SurfaceIso::South => 4,
                    SurfaceIso::East => 5,
                    SurfaceIso::North => 6,
                });
                for tolerance in trim.tolerance() {
                    writer.f64(tolerance);
                }
                writer.curve2(trim.curve())?;
            }
        }
    }
    Ok(writer.bytes)
}

pub(crate) fn decode_brep(bytes: &[u8], tolerance: Tolerance) -> Result<Brep, GeometryCodecError> {
    let mut reader = Reader { bytes, position: 0 };
    if reader.take(MAGIC.len())? != MAGIC || reader.u32()? != VERSION {
        return Err(GeometryCodecError::Malformed);
    }
    let vertex_count = reader.len()?;
    let edge_count = reader.len()?;
    let face_count = reader.len()?;
    let mut vertices = Vec::with_capacity(vertex_count);
    let mut edges = Vec::with_capacity(edge_count);
    let mut faces = Vec::with_capacity(face_count);

    for _ in 0..vertex_count {
        vertices.push(BrepVertex::try_new(reader.point3()?, reader.f64()?)?);
    }
    for _ in 0..edge_count {
        let vertices = [reader.index()?, reader.index()?];
        let edge_tolerance = reader.f64()?;
        let curve = reader.curve3()?;
        edges.push(BrepEdge::try_new(vertices, curve, edge_tolerance)?);
    }
    for _ in 0..face_count {
        let reversed = reader.boolean()?;
        let surface = reader.surface()?;
        let loop_count = reader.len()?;
        let mut loops = Vec::with_capacity(loop_count);
        for _ in 0..loop_count {
            let loop_type = match reader.u8()? {
                1 => BrepLoopType::Outer,
                2 => BrepLoopType::Inner,
                _ => return Err(GeometryCodecError::Malformed),
            };
            let trim_count = reader.len()?;
            let mut trims = Vec::with_capacity(trim_count);
            for _ in 0..trim_count {
                let vertices = [reader.index()?, reader.index()?];
                let edge = match reader.u64()? {
                    NO_EDGE => None,
                    index => {
                        Some(usize::try_from(index).map_err(|_| GeometryCodecError::SizeOverflow)?)
                    }
                };
                let reversed_3d = reader.boolean()?;
                let trim_type = match reader.u8()? {
                    1 => BrepTrimType::Boundary,
                    2 => BrepTrimType::Mated,
                    3 => BrepTrimType::Seam,
                    4 => BrepTrimType::Singular,
                    _ => return Err(GeometryCodecError::Malformed),
                };
                let iso = match reader.u8()? {
                    0 => SurfaceIso::NotIso,
                    1 => SurfaceIso::InteriorUConstant,
                    2 => SurfaceIso::InteriorVConstant,
                    3 => SurfaceIso::West,
                    4 => SurfaceIso::South,
                    5 => SurfaceIso::East,
                    6 => SurfaceIso::North,
                    _ => return Err(GeometryCodecError::Malformed),
                };
                let trim_tolerance = [reader.f64()?, reader.f64()?];
                let curve = reader.curve2()?;
                trims.push(BrepTrim::try_new(
                    vertices,
                    edge,
                    reversed_3d,
                    curve,
                    trim_type,
                    iso,
                    trim_tolerance,
                )?);
            }
            loops.push(BrepLoop::try_new(loop_type, trims)?);
        }
        faces.push(BrepFace::try_new(surface, reversed, loops)?);
    }
    if reader.position != bytes.len() {
        return Err(GeometryCodecError::Malformed);
    }
    Ok(Brep::try_new(vertices, edges, faces, tolerance)?)
}

#[derive(Default)]
struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn boolean(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn f64(&mut self, value: f64) {
        self.u64(value.to_bits());
    }

    fn len(&mut self, value: usize) -> Result<(), GeometryCodecError> {
        self.u64(u64::try_from(value).map_err(|_| GeometryCodecError::SizeOverflow)?);
        Ok(())
    }

    fn index(&mut self, value: usize) -> Result<(), GeometryCodecError> {
        self.len(value)
    }

    fn point3(&mut self, point: Point3) {
        for coordinate in point.to_array() {
            self.f64(coordinate);
        }
    }

    fn curve3(&mut self, curve: &NurbsCurve) -> Result<(), GeometryCodecError> {
        self.u32(u32::try_from(curve.degree()).map_err(|_| GeometryCodecError::SizeOverflow)?);
        self.len(curve.control_points().len())?;
        self.len(curve.knots().len())?;
        for control in curve.control_points() {
            self.point3(control.point());
            self.f64(control.weight());
        }
        for knot in curve.knots() {
            self.f64(*knot);
        }
        Ok(())
    }

    fn segment(&mut self, segment: &CurveSegment3) -> Result<(), GeometryCodecError> {
        match segment {
            CurveSegment3::Line(line) => {
                self.u8(1);
                self.point3(line.start());
                self.point3(line.end());
                self.f64(*line.domain().start());
                self.f64(*line.domain().end());
            }
            CurveSegment3::Arc(arc) => {
                self.u8(2);
                self.point3(arc.center());
                for value in arc.x_axis().as_vector().to_array() {
                    self.f64(value);
                }
                for value in arc.normal()?.as_vector().to_array() {
                    self.f64(value);
                }
                self.f64(arc.radius());
                self.f64(arc.sweep_radians());
                self.f64(*arc.domain().start());
                self.f64(*arc.domain().end());
            }
            CurveSegment3::Polyline(polyline) => {
                self.u8(3);
                self.len(polyline.vertices().len())?;
                for point in polyline.vertices() {
                    self.point3(*point);
                }
                for parameter in polyline.parameters() {
                    self.f64(*parameter);
                }
            }
            CurveSegment3::NurbsCurve(curve) => {
                self.u8(4);
                self.curve3(curve)?;
            }
        }
        Ok(())
    }

    fn curve2(&mut self, curve: &NurbsCurve2) -> Result<(), GeometryCodecError> {
        self.u32(u32::try_from(curve.degree()).map_err(|_| GeometryCodecError::SizeOverflow)?);
        self.len(curve.control_points().len())?;
        self.len(curve.knots().len())?;
        for control in curve.control_points() {
            self.f64(control.point().x());
            self.f64(control.point().y());
            self.f64(control.weight());
        }
        for knot in curve.knots() {
            self.f64(*knot);
        }
        Ok(())
    }

    fn surface(&mut self, surface: &NurbsSurface) -> Result<(), GeometryCodecError> {
        self.u32(u32::try_from(surface.degree_u()).map_err(|_| GeometryCodecError::SizeOverflow)?);
        self.u32(u32::try_from(surface.degree_v()).map_err(|_| GeometryCodecError::SizeOverflow)?);
        self.len(surface.control_point_count_u())?;
        self.len(surface.control_point_count_v())?;
        self.len(surface.knots_u().len())?;
        self.len(surface.knots_v().len())?;
        for control in surface.control_points() {
            self.point3(control.point());
            self.f64(control.weight());
        }
        for knot in surface.knots_u() {
            self.f64(*knot);
        }
        for knot in surface.knots_v() {
            self.f64(*knot);
        }
        Ok(())
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl Reader<'_> {
    fn take(&mut self, count: usize) -> Result<&[u8], GeometryCodecError> {
        let end = self
            .position
            .checked_add(count)
            .filter(|end| *end <= self.bytes.len())
            .ok_or(GeometryCodecError::Malformed)?;
        let values = &self.bytes[self.position..end];
        self.position = end;
        Ok(values)
    }

    fn u8(&mut self) -> Result<u8, GeometryCodecError> {
        Ok(self.take(1)?[0])
    }

    fn boolean(&mut self) -> Result<bool, GeometryCodecError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(GeometryCodecError::Malformed),
        }
    }

    fn u32(&mut self) -> Result<u32, GeometryCodecError> {
        Ok(u32::from_le_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| GeometryCodecError::Malformed)?,
        ))
    }

    fn u64(&mut self) -> Result<u64, GeometryCodecError> {
        Ok(u64::from_le_bytes(
            self.take(8)?
                .try_into()
                .map_err(|_| GeometryCodecError::Malformed)?,
        ))
    }

    fn f64(&mut self) -> Result<f64, GeometryCodecError> {
        Ok(f64::from_bits(self.u64()?))
    }

    fn len(&mut self) -> Result<usize, GeometryCodecError> {
        let count = usize::try_from(self.u64()?).map_err(|_| GeometryCodecError::SizeOverflow)?;
        if count > self.bytes.len().saturating_sub(self.position) {
            return Err(GeometryCodecError::Malformed);
        }
        Ok(count)
    }

    fn index(&mut self) -> Result<usize, GeometryCodecError> {
        usize::try_from(self.u64()?).map_err(|_| GeometryCodecError::SizeOverflow)
    }

    fn point3(&mut self) -> Result<Point3, GeometryCodecError> {
        Ok(Point3::try_new(self.f64()?, self.f64()?, self.f64()?)?)
    }

    fn require_f64s(&self, count: usize) -> Result<(), GeometryCodecError> {
        if count > (self.bytes.len() - self.position) / size_of::<f64>() {
            return Err(GeometryCodecError::Malformed);
        }
        Ok(())
    }

    fn require_control_payload(
        &self,
        controls: usize,
        dimension: usize,
        knots: usize,
    ) -> Result<(), GeometryCodecError> {
        let count = controls
            .checked_mul(dimension + 1)
            .and_then(|count| count.checked_add(knots))
            .ok_or(GeometryCodecError::SizeOverflow)?;
        self.require_f64s(count)
    }

    fn curve3(&mut self) -> Result<NurbsCurve, GeometryCodecError> {
        let degree = usize::try_from(self.u32()?).map_err(|_| GeometryCodecError::SizeOverflow)?;
        let control_count = self.len()?;
        let knot_count = self.len()?;
        self.require_control_payload(control_count, 3, knot_count)?;
        let mut controls = Vec::with_capacity(control_count);
        for _ in 0..control_count {
            controls.push(WeightedPoint3::try_new(self.point3()?, self.f64()?)?);
        }
        let mut knots = Vec::with_capacity(knot_count);
        for _ in 0..knot_count {
            knots.push(self.f64()?);
        }
        Ok(NurbsCurve::try_new_rational(degree, controls, knots)?)
    }

    fn segment(&mut self) -> Result<CurveSegment3, GeometryCodecError> {
        let tolerance = Tolerance::try_new(
            f64::MIN_POSITIVE,
            Tolerance::DEFAULT.relative(),
            Tolerance::DEFAULT.angular(),
        )?;
        Ok(match self.u8()? {
            1 => {
                let line = LineSegment::try_new(self.point3()?, self.point3()?, tolerance)?;
                CurveSegment3::Line(line.try_reparameterized(self.f64()?..=self.f64()?)?)
            }
            2 => {
                let center = self.point3()?;
                let x = Vector3::try_new(self.f64()?, self.f64()?, self.f64()?)?
                    .normalized_nonzero()?;
                let normal = Vector3::try_new(self.f64()?, self.f64()?, self.f64()?)?
                    .normalized_nonzero()?;
                let circle = Circle3::try_from_frame(center, self.f64()?, x, normal, tolerance)?;
                let arc = CircularArc3::try_from_circle_sweep(circle, self.f64()?)?;
                CurveSegment3::Arc(arc.try_reparameterized(self.f64()?..=self.f64()?)?)
            }
            3 => {
                let count = self.len()?;
                self.require_f64s(
                    count
                        .checked_mul(4)
                        .ok_or(GeometryCodecError::SizeOverflow)?,
                )?;
                let points = (0..count)
                    .map(|_| self.point3())
                    .collect::<Result<Vec<_>, _>>()?;
                let parameters = (0..count)
                    .map(|_| self.f64())
                    .collect::<Result<Vec<_>, _>>()?;
                CurveSegment3::Polyline(Polyline3::try_with_parameters(
                    points, parameters, tolerance,
                )?)
            }
            4 => CurveSegment3::NurbsCurve(self.curve3()?),
            _ => return Err(GeometryCodecError::Malformed),
        })
    }

    fn curve2(&mut self) -> Result<NurbsCurve2, GeometryCodecError> {
        let degree = usize::try_from(self.u32()?).map_err(|_| GeometryCodecError::SizeOverflow)?;
        let control_count = self.len()?;
        let knot_count = self.len()?;
        self.require_control_payload(control_count, 2, knot_count)?;
        let mut controls = Vec::with_capacity(control_count);
        for _ in 0..control_count {
            controls.push(WeightedPoint2::try_new(
                Point2::try_new(self.f64()?, self.f64()?)?,
                self.f64()?,
            )?);
        }
        let mut knots = Vec::with_capacity(knot_count);
        for _ in 0..knot_count {
            knots.push(self.f64()?);
        }
        Ok(NurbsCurve2::try_new_rational(degree, controls, knots)?)
    }

    fn surface(&mut self) -> Result<NurbsSurface, GeometryCodecError> {
        let degree_u =
            usize::try_from(self.u32()?).map_err(|_| GeometryCodecError::SizeOverflow)?;
        let degree_v =
            usize::try_from(self.u32()?).map_err(|_| GeometryCodecError::SizeOverflow)?;
        let count_u = self.len()?;
        let count_v = self.len()?;
        let knot_count_u = self.len()?;
        let knot_count_v = self.len()?;
        let control_count = count_u
            .checked_mul(count_v)
            .ok_or(GeometryCodecError::SizeOverflow)?;
        let knot_count = knot_count_u
            .checked_add(knot_count_v)
            .ok_or(GeometryCodecError::SizeOverflow)?;
        self.require_control_payload(control_count, 3, knot_count)?;
        let mut controls = Vec::with_capacity(control_count);
        for _ in 0..control_count {
            controls.push(WeightedPoint3::try_new(self.point3()?, self.f64()?)?);
        }
        let mut knots_u = Vec::with_capacity(knot_count_u);
        for _ in 0..knot_count_u {
            knots_u.push(self.f64()?);
        }
        let mut knots_v = Vec::with_capacity(knot_count_v);
        for _ in 0..knot_count_v {
            knots_v.push(self.f64()?);
        }
        Ok(NurbsSurface::try_new_rational(
            degree_u, degree_v, count_u, count_v, controls, knots_u, knots_v,
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::{Frame3, Vector3};

    #[test]
    fn legacy_polycurve_payload_remains_readable_and_arc_payload_is_strict() {
        let line = LineSegment::try_new(
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let mut writer = Writer::default();
        writer.bytes.extend_from_slice(POLYCURVE_MAGIC);
        writer.u32(1);
        writer.len(1).unwrap();
        writer.f64(-3.0);
        writer.f64(9.0);
        writer.curve3(&line).unwrap();
        let decoded = decode_polycurve(&writer.bytes).unwrap();
        assert_eq!(decoded.parameters(), &[-3.0, 9.0]);
        assert_eq!(decoded.segments(), &[CurveSegment3::NurbsCurve(line)]);
        let circle = Circle3::try_new(
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            2.0,
            Vector3::try_new(0.0, 0.0, 1.0)
                .unwrap()
                .normalized_nonzero()
                .unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let arc = CircularArc3::try_from_circle_sweep(circle, 1.0).unwrap();
        let bytes = encode_arc(arc).unwrap();
        assert_eq!(decode_arc(&bytes).unwrap(), arc);
        for length in 0..bytes.len() {
            assert!(decode_arc(&bytes[..length]).is_err());
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(decode_arc(&trailing).is_err());
        let mut wrong_type = bytes;
        wrong_type[4] = 99;
        assert!(decode_arc(&wrong_type).is_err());
    }

    #[test]
    fn polycurve_codec_preserves_local_domains_and_rejects_malformed_data() {
        let line = NurbsCurve::try_new(
            1,
            vec![
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            ],
            vec![4.0, 4.0, 9.0, 9.0],
        )
        .unwrap();
        let curve = PolyCurve3::try_with_segment_domains(vec![line], vec![-3.0, 7.0]).unwrap();
        let bytes = encode_polycurve(&curve).unwrap();
        assert_eq!(decode_polycurve(&bytes).unwrap(), curve);
        for count in 0..bytes.len() {
            assert!(
                decode_polycurve(&bytes[..count]).is_err(),
                "truncation at {count}"
            );
        }
        let mut invalid = bytes.clone();
        invalid.push(0);
        assert!(decode_polycurve(&invalid).is_err());
        for (range, value) in [
            (12..20, 0_u64.to_le_bytes()),
            (12..20, u64::MAX.to_le_bytes()),
            (20..28, f64::NAN.to_le_bytes()),
            (28..36, (-3.0_f64).to_le_bytes()),
        ] {
            let mut invalid = bytes.clone();
            invalid[range].copy_from_slice(&value);
            assert!(decode_polycurve(&invalid).is_err());
        }
    }

    #[test]
    fn versioned_payload_round_trips_exact_solid_topology() {
        let frame = Frame3::try_from_normal(
            Point3::try_new(1.0, 2.0, 3.0).unwrap(),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let rectangular_trim = Brep::try_rectangular_surface_face(
            NurbsSurface::try_bilinear([
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(10.0, 0.0, 1.0).unwrap(),
                Point3::try_new(10.0, 10.0, 3.0).unwrap(),
                Point3::try_new(0.0, 10.0, -1.0).unwrap(),
            ])
            .unwrap(),
            0.0..=0.4,
            0.0..=0.6,
            Tolerance::DEFAULT,
        )
        .unwrap();
        for brep in [
            Brep::try_box(
                frame,
                [[-1.0, 2.0], [-2.0, 3.0], [-3.0, 4.0]],
                Tolerance::DEFAULT,
            )
            .unwrap(),
            Brep::try_cylinder(frame, 2.5, -3.0, 4.0, Tolerance::DEFAULT).unwrap(),
            Brep::try_cone(frame, 2.5, -4.0, Tolerance::DEFAULT).unwrap(),
            rectangular_trim,
        ] {
            let payload = encode_brep(&brep).unwrap();
            assert_eq!(decode_brep(&payload, Tolerance::DEFAULT).unwrap(), brep);
        }
    }

    #[test]
    fn malformed_or_trailing_payload_is_rejected() {
        let frame = Frame3::try_from_normal(
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let brep = Brep::try_cone(frame, 1.0, 2.0, Tolerance::DEFAULT).unwrap();
        let mut payload = encode_brep(&brep).unwrap();
        payload.push(0);
        assert!(matches!(
            decode_brep(&payload, Tolerance::DEFAULT),
            Err(GeometryCodecError::Malformed)
        ));
        assert!(matches!(
            decode_brep(&payload[..12], Tolerance::DEFAULT),
            Err(GeometryCodecError::Malformed)
        ));
    }
}

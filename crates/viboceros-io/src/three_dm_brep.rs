use viboceros_geometry::{
    Brep, BrepEdge, BrepFace, BrepLoop, BrepLoopType, BrepTrim, BrepTrimType, BrepVertex,
    GeometryError, NurbsCurve, NurbsCurve2, NurbsSurface, Point2, Point3, SurfaceIso, Tolerance,
    WeightedPoint2, WeightedPoint3,
};

const MAGIC: &[u8; 8] = b"VIBOBRP\0";
const VERSION: u32 = 1;
const NO_EDGE: u64 = u64::MAX;

#[derive(Debug)]
pub(crate) enum BrepCodecError {
    Malformed,
    SizeOverflow,
    Geometry(GeometryError),
}

impl From<GeometryError> for BrepCodecError {
    fn from(error: GeometryError) -> Self {
        Self::Geometry(error)
    }
}

pub(crate) fn encode(brep: &Brep) -> Result<Vec<u8>, BrepCodecError> {
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
                    Some(edge) => u64::try_from(edge).map_err(|_| BrepCodecError::SizeOverflow)?,
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

pub(crate) fn decode(bytes: &[u8], tolerance: Tolerance) -> Result<Brep, BrepCodecError> {
    let mut reader = Reader { bytes, position: 0 };
    if reader.take(MAGIC.len())? != MAGIC || reader.u32()? != VERSION {
        return Err(BrepCodecError::Malformed);
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
                _ => return Err(BrepCodecError::Malformed),
            };
            let trim_count = reader.len()?;
            let mut trims = Vec::with_capacity(trim_count);
            for _ in 0..trim_count {
                let vertices = [reader.index()?, reader.index()?];
                let edge = match reader.u64()? {
                    NO_EDGE => None,
                    index => {
                        Some(usize::try_from(index).map_err(|_| BrepCodecError::SizeOverflow)?)
                    }
                };
                let reversed_3d = reader.boolean()?;
                let trim_type = match reader.u8()? {
                    1 => BrepTrimType::Boundary,
                    2 => BrepTrimType::Mated,
                    3 => BrepTrimType::Seam,
                    4 => BrepTrimType::Singular,
                    _ => return Err(BrepCodecError::Malformed),
                };
                let iso = match reader.u8()? {
                    0 => SurfaceIso::NotIso,
                    1 => SurfaceIso::InteriorUConstant,
                    2 => SurfaceIso::InteriorVConstant,
                    3 => SurfaceIso::West,
                    4 => SurfaceIso::South,
                    5 => SurfaceIso::East,
                    6 => SurfaceIso::North,
                    _ => return Err(BrepCodecError::Malformed),
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
        return Err(BrepCodecError::Malformed);
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

    fn len(&mut self, value: usize) -> Result<(), BrepCodecError> {
        self.u64(u64::try_from(value).map_err(|_| BrepCodecError::SizeOverflow)?);
        Ok(())
    }

    fn index(&mut self, value: usize) -> Result<(), BrepCodecError> {
        self.len(value)
    }

    fn point3(&mut self, point: Point3) {
        for coordinate in point.to_array() {
            self.f64(coordinate);
        }
    }

    fn curve3(&mut self, curve: &NurbsCurve) -> Result<(), BrepCodecError> {
        self.u32(u32::try_from(curve.degree()).map_err(|_| BrepCodecError::SizeOverflow)?);
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

    fn curve2(&mut self, curve: &NurbsCurve2) -> Result<(), BrepCodecError> {
        self.u32(u32::try_from(curve.degree()).map_err(|_| BrepCodecError::SizeOverflow)?);
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

    fn surface(&mut self, surface: &NurbsSurface) -> Result<(), BrepCodecError> {
        self.u32(u32::try_from(surface.degree_u()).map_err(|_| BrepCodecError::SizeOverflow)?);
        self.u32(u32::try_from(surface.degree_v()).map_err(|_| BrepCodecError::SizeOverflow)?);
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
    fn take(&mut self, count: usize) -> Result<&[u8], BrepCodecError> {
        let end = self
            .position
            .checked_add(count)
            .filter(|end| *end <= self.bytes.len())
            .ok_or(BrepCodecError::Malformed)?;
        let values = &self.bytes[self.position..end];
        self.position = end;
        Ok(values)
    }

    fn u8(&mut self) -> Result<u8, BrepCodecError> {
        Ok(self.take(1)?[0])
    }

    fn boolean(&mut self) -> Result<bool, BrepCodecError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(BrepCodecError::Malformed),
        }
    }

    fn u32(&mut self) -> Result<u32, BrepCodecError> {
        Ok(u32::from_le_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| BrepCodecError::Malformed)?,
        ))
    }

    fn u64(&mut self) -> Result<u64, BrepCodecError> {
        Ok(u64::from_le_bytes(
            self.take(8)?
                .try_into()
                .map_err(|_| BrepCodecError::Malformed)?,
        ))
    }

    fn f64(&mut self) -> Result<f64, BrepCodecError> {
        Ok(f64::from_bits(self.u64()?))
    }

    fn len(&mut self) -> Result<usize, BrepCodecError> {
        let count = usize::try_from(self.u64()?).map_err(|_| BrepCodecError::SizeOverflow)?;
        if count > self.bytes.len().saturating_sub(self.position) {
            return Err(BrepCodecError::Malformed);
        }
        Ok(count)
    }

    fn index(&mut self) -> Result<usize, BrepCodecError> {
        usize::try_from(self.u64()?).map_err(|_| BrepCodecError::SizeOverflow)
    }

    fn point3(&mut self) -> Result<Point3, BrepCodecError> {
        Ok(Point3::try_new(self.f64()?, self.f64()?, self.f64()?)?)
    }

    fn curve3(&mut self) -> Result<NurbsCurve, BrepCodecError> {
        let degree = usize::try_from(self.u32()?).map_err(|_| BrepCodecError::SizeOverflow)?;
        let control_count = self.len()?;
        let knot_count = self.len()?;
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

    fn curve2(&mut self) -> Result<NurbsCurve2, BrepCodecError> {
        let degree = usize::try_from(self.u32()?).map_err(|_| BrepCodecError::SizeOverflow)?;
        let control_count = self.len()?;
        let knot_count = self.len()?;
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

    fn surface(&mut self) -> Result<NurbsSurface, BrepCodecError> {
        let degree_u = usize::try_from(self.u32()?).map_err(|_| BrepCodecError::SizeOverflow)?;
        let degree_v = usize::try_from(self.u32()?).map_err(|_| BrepCodecError::SizeOverflow)?;
        let count_u = self.len()?;
        let count_v = self.len()?;
        let knot_count_u = self.len()?;
        let knot_count_v = self.len()?;
        let control_count = count_u
            .checked_mul(count_v)
            .ok_or(BrepCodecError::SizeOverflow)?;
        if control_count > self.bytes.len().saturating_sub(self.position) {
            return Err(BrepCodecError::Malformed);
        }
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
            let payload = encode(&brep).unwrap();
            assert_eq!(decode(&payload, Tolerance::DEFAULT).unwrap(), brep);
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
        let mut payload = encode(&brep).unwrap();
        payload.push(0);
        assert!(matches!(
            decode(&payload, Tolerance::DEFAULT),
            Err(BrepCodecError::Malformed)
        ));
        assert!(matches!(
            decode(&payload[..12], Tolerance::DEFAULT),
            Err(BrepCodecError::Malformed)
        ));
    }
}

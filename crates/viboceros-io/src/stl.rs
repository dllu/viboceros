use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

use thiserror::Error;
use viboceros_geometry::{GeometryError, Point3, Real, Tolerance, TriangleMesh, UnitVector3};

const BINARY_HEADER_SIZE: u64 = 80;
const BINARY_PREFIX_SIZE: u64 = BINARY_HEADER_SIZE + 4;
const BINARY_FACET_SIZE: u64 = 50;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StlFormat {
    Ascii,
    Binary,
}

#[derive(Debug, Error)]
pub enum StlError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Geometry(#[from] GeometryError),

    #[error("malformed ASCII STL at line {line}: {context}")]
    MalformedAscii { line: usize, context: &'static str },

    #[error("invalid finite number '{value}' at ASCII STL line {line}")]
    InvalidNumber { line: usize, value: String },

    #[error("binary STL declares {declared} bytes but the file contains {actual}")]
    BinaryLengthMismatch { declared: u64, actual: u64 },

    #[error("STL contains too many triangles for 32-bit mesh indices")]
    TooManyTriangles,

    #[error("value {value} cannot be represented by binary STL's 32-bit floats")]
    F32OutOfRange { value: Real },

    #[error("binary STL triangle {triangle} has a non-finite {field} component")]
    NonFiniteBinaryValue {
        triangle: usize,
        field: &'static str,
    },

    #[error("triangle {triangle} becomes degenerate at binary STL's 32-bit precision")]
    BinaryPrecisionLoss { triangle: usize },
}

pub fn read_stl<R: Read + Seek>(
    mut reader: R,
    tolerance: Tolerance,
) -> Result<TriangleMesh, StlError> {
    let file_length = reader.seek(SeekFrom::End(0))?;
    reader.seek(SeekFrom::Start(0))?;

    let mut prefix = [0_u8; BINARY_PREFIX_SIZE as usize];
    let prefix_length = usize::try_from(file_length.min(BINARY_PREFIX_SIZE))
        .expect("the binary STL prefix length fits usize");
    reader.read_exact(&mut prefix[..prefix_length])?;
    let binary_length = if prefix_length == prefix.len() {
        let facet_count = u32::from_le_bytes(prefix[80..84].try_into().unwrap());
        Some(
            BINARY_PREFIX_SIZE
                .checked_add(BINARY_FACET_SIZE * u64::from(facet_count))
                .ok_or(StlError::TooManyTriangles)?,
        )
    } else {
        None
    };
    reader.seek(SeekFrom::Start(0))?;

    if binary_length == Some(file_length) {
        return read_binary_stl(BufReader::new(reader), tolerance);
    }

    let starts_with_solid = prefix[..prefix_length]
        .iter()
        .copied()
        .skip_while(u8::is_ascii_whitespace)
        .take(5)
        .map(|byte| byte.to_ascii_lowercase())
        .eq(b"solid".iter().copied());
    if starts_with_solid {
        read_ascii_stl(BufReader::new(reader), tolerance)
    } else if let Some(declared) = binary_length {
        Err(StlError::BinaryLengthMismatch {
            declared,
            actual: file_length,
        })
    } else {
        Err(StlError::BinaryLengthMismatch {
            declared: BINARY_PREFIX_SIZE,
            actual: file_length,
        })
    }
}

pub fn read_stl_file(
    path: impl AsRef<Path>,
    tolerance: Tolerance,
) -> Result<TriangleMesh, StlError> {
    read_stl(File::open(path)?, tolerance)
}

pub fn write_stl<W: Write>(
    writer: W,
    mesh: &TriangleMesh,
    format: StlFormat,
) -> Result<(), StlError> {
    validate_stl_for_write(mesh, format)?;
    write_validated_stl(writer, mesh, format)
}

fn write_validated_stl<W: Write>(
    writer: W,
    mesh: &TriangleMesh,
    format: StlFormat,
) -> Result<(), StlError> {
    match format {
        StlFormat::Ascii => write_ascii_stl(writer, mesh),
        StlFormat::Binary => write_binary_stl(writer, mesh),
    }
}

pub fn write_stl_file(
    path: impl AsRef<Path>,
    mesh: &TriangleMesh,
    format: StlFormat,
) -> Result<(), StlError> {
    validate_stl_for_write(mesh, format)?;
    let mut writer = BufWriter::new(File::create(path)?);
    write_validated_stl(&mut writer, mesh, format)?;
    writer.flush()?;
    Ok(())
}

fn read_binary_stl<R: Read>(mut reader: R, tolerance: Tolerance) -> Result<TriangleMesh, StlError> {
    let mut header = [0_u8; BINARY_HEADER_SIZE as usize];
    reader.read_exact(&mut header)?;
    let triangle_count = read_u32(&mut reader)? as usize;
    if triangle_count > (u32::MAX as usize) / 3 {
        return Err(StlError::TooManyTriangles);
    }
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    vertices
        .try_reserve_exact(triangle_count * 3)
        .map_err(|_| StlError::TooManyTriangles)?;
    triangles
        .try_reserve_exact(triangle_count)
        .map_err(|_| StlError::TooManyTriangles)?;

    for triangle_index in 0..triangle_count {
        for _ in 0..3 {
            read_finite_f32(&mut reader, triangle_index, "normal")?;
        }
        let base = (triangle_index * 3) as u32;
        for _ in 0..3 {
            vertices.push(Point3::try_new(
                Real::from(read_finite_f32(&mut reader, triangle_index, "vertex")?),
                Real::from(read_finite_f32(&mut reader, triangle_index, "vertex")?),
                Real::from(read_finite_f32(&mut reader, triangle_index, "vertex")?),
            )?);
        }
        let mut attributes = [0_u8; 2];
        reader.read_exact(&mut attributes)?;
        triangles.push([base, base + 1, base + 2]);
    }

    Ok(TriangleMesh::try_new(vertices, triangles, tolerance)?)
}

fn read_ascii_stl<R: BufRead>(reader: R, tolerance: Tolerance) -> Result<TriangleMesh, StlError> {
    #[derive(Clone, Copy)]
    enum State {
        Start,
        Solid,
        Facet,
        Loop(usize),
        EndLoop,
        Finished,
    }

    let mut state = State::Start;
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    let mut facet_vertices = [Point3::try_new(0.0, 0.0, 0.0).unwrap(); 3];
    let mut last_line = 0;

    for (line_index, line) in reader.lines().enumerate() {
        let line_number = line_index + 1;
        last_line = line_number;
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let tokens: Vec<_> = trimmed.split_whitespace().collect();
        let keyword = tokens[0].to_ascii_lowercase();
        state = match state {
            State::Start if keyword == "solid" => State::Solid,
            State::Solid if keyword == "facet" => {
                if tokens.len() == 5 && tokens[1].eq_ignore_ascii_case("normal") {
                    for value in &tokens[2..5] {
                        parse_real(value, line_number)?;
                    }
                } else if tokens.len() != 1 {
                    return malformed(line_number, "expected 'facet normal nx ny nz'");
                }
                State::Facet
            }
            State::Solid if keyword == "endsolid" => State::Finished,
            State::Facet
                if tokens.len() == 2
                    && tokens[0].eq_ignore_ascii_case("outer")
                    && tokens[1].eq_ignore_ascii_case("loop") =>
            {
                State::Loop(0)
            }
            State::Loop(vertex_count) if keyword == "vertex" && tokens.len() == 4 => {
                if vertex_count >= 3 {
                    return malformed(line_number, "a facet must contain exactly three vertices");
                }
                facet_vertices[vertex_count] = Point3::try_new(
                    parse_real(tokens[1], line_number)?,
                    parse_real(tokens[2], line_number)?,
                    parse_real(tokens[3], line_number)?,
                )?;
                State::Loop(vertex_count + 1)
            }
            State::Loop(3) if keyword == "endloop" && tokens.len() == 1 => State::EndLoop,
            State::EndLoop if keyword == "endfacet" && tokens.len() == 1 => {
                let base = u32::try_from(vertices.len()).map_err(|_| StlError::TooManyTriangles)?;
                if base > u32::MAX - 3 {
                    return Err(StlError::TooManyTriangles);
                }
                vertices.extend(facet_vertices);
                triangles.push([base, base + 1, base + 2]);
                State::Solid
            }
            State::Finished => {
                return malformed(line_number, "content appears after 'endsolid'");
            }
            _ => return malformed(line_number, "unexpected record for the current STL state"),
        };
    }

    if !matches!(state, State::Finished) {
        return malformed(last_line.max(1), "file ended before 'endsolid'");
    }
    Ok(TriangleMesh::try_new(vertices, triangles, tolerance)?)
}

fn write_binary_stl<W: Write>(mut writer: W, mesh: &TriangleMesh) -> Result<(), StlError> {
    let mut header = [0_u8; BINARY_HEADER_SIZE as usize];
    let description = b"Viboceros binary STL";
    header[..description.len()].copy_from_slice(description);
    writer.write_all(&header)?;
    let count = u32::try_from(mesh.triangles().len()).map_err(|_| StlError::TooManyTriangles)?;
    writer.write_all(&count.to_le_bytes())?;

    for triangle_index in 0..mesh.triangles().len() {
        let (points, normal) = quantized_triangle(mesh, triangle_index)?;
        for component in [normal.x(), normal.y(), normal.z()] {
            writer.write_all(&quantize_f32(component)?.to_le_bytes())?;
        }
        for point in points {
            for coordinate in point {
                writer.write_all(&coordinate.to_le_bytes())?;
            }
        }
        writer.write_all(&0_u16.to_le_bytes())?;
    }
    Ok(())
}

fn write_ascii_stl<W: Write>(mut writer: W, mesh: &TriangleMesh) -> Result<(), StlError> {
    writeln!(writer, "solid viboceros")?;
    for triangle_index in 0..mesh.triangles().len() {
        let normal = mesh.face_normal(triangle_index)?;
        writeln!(
            writer,
            "  facet normal {:.17e} {:.17e} {:.17e}",
            normal.x(),
            normal.y(),
            normal.z()
        )?;
        writeln!(writer, "    outer loop")?;
        for point in mesh.triangle_points(triangle_index).unwrap() {
            writeln!(
                writer,
                "      vertex {:.17e} {:.17e} {:.17e}",
                point.x(),
                point.y(),
                point.z()
            )?;
        }
        writeln!(writer, "    endloop")?;
        writeln!(writer, "  endfacet")?;
    }
    writeln!(writer, "endsolid viboceros")?;
    Ok(())
}

fn malformed<T>(line: usize, context: &'static str) -> Result<T, StlError> {
    Err(StlError::MalformedAscii { line, context })
}

fn parse_real(value: &str, line: usize) -> Result<Real, StlError> {
    let parsed = value.parse::<Real>().map_err(|_| StlError::InvalidNumber {
        line,
        value: value.to_owned(),
    })?;
    if parsed.is_finite() {
        Ok(parsed)
    } else {
        Err(StlError::InvalidNumber {
            line,
            value: value.to_owned(),
        })
    }
}

fn read_u32(reader: &mut impl Read) -> Result<u32, std::io::Error> {
    let mut bytes = [0_u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_f32(reader: &mut impl Read) -> Result<f32, std::io::Error> {
    let mut bytes = [0_u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(f32::from_le_bytes(bytes))
}

fn read_finite_f32(
    reader: &mut impl Read,
    triangle: usize,
    field: &'static str,
) -> Result<f32, StlError> {
    let value = read_f32(reader)?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(StlError::NonFiniteBinaryValue { triangle, field })
    }
}

fn quantize_f32(value: Real) -> Result<f32, StlError> {
    let value32 = value as f32;
    if !value32.is_finite() {
        return Err(StlError::F32OutOfRange { value });
    }
    Ok(value32)
}

fn validate_stl_for_write(mesh: &TriangleMesh, format: StlFormat) -> Result<(), StlError> {
    u32::try_from(mesh.triangles().len()).map_err(|_| StlError::TooManyTriangles)?;
    for triangle in 0..mesh.triangles().len() {
        match format {
            StlFormat::Ascii => {
                mesh.face_normal(triangle)?;
            }
            StlFormat::Binary => {
                quantized_triangle(mesh, triangle)?;
            }
        }
    }
    Ok(())
}

fn quantized_triangle(
    mesh: &TriangleMesh,
    triangle: usize,
) -> Result<([[f32; 3]; 3], UnitVector3), StlError> {
    let source = mesh
        .triangle_points(triangle)
        .ok_or(GeometryError::TriangleIndexOutOfRange { triangle })?;
    let mut quantized = [[0.0_f32; 3]; 3];
    for (target, point) in quantized.iter_mut().zip(source) {
        for (coordinate, value) in target.iter_mut().zip(point.to_array()) {
            *coordinate = quantize_f32(value)?;
        }
    }

    let points = quantized.map(|point| {
        Point3::try_new(
            Real::from(point[0]),
            Real::from(point[1]),
            Real::from(point[2]),
        )
        .expect("finite f32 coordinates are valid Real points")
    });
    let minimum_tolerance =
        Tolerance::try_new(Real::MIN_POSITIVE, Real::MIN_POSITIVE, Real::MIN_POSITIVE)
            .expect("positive finite tolerance components");
    let normal = (|| {
        let first = points[0]
            .vector_to(points[1])?
            .normalized(minimum_tolerance)?;
        let second = points[0]
            .vector_to(points[2])?
            .normalized(minimum_tolerance)?;
        first
            .as_vector()
            .cross(second.as_vector())?
            .normalized(minimum_tolerance)
    })()
    .map_err(|_: GeometryError| StlError::BinaryPrecisionLoss { triangle })?;

    Ok((quantized, normal))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{Cursor, Read, Seek, SeekFrom};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn mesh() -> TriangleMesh {
        TriangleMesh::try_new(
            vec![
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(1.0, 0.0, 0.0).unwrap(),
                Point3::try_new(0.0, 1.0, 0.0).unwrap(),
                Point3::try_new(1.0, 1.0, 0.0).unwrap(),
            ],
            vec![[0, 1, 2], [1, 3, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap()
    }

    #[test]
    fn binary_round_trip_and_solid_header_detection() {
        let original = mesh();
        let mut bytes = Vec::new();
        write_stl(&mut bytes, &original, StlFormat::Binary).unwrap();
        bytes[..5].copy_from_slice(b"solid");
        assert_eq!(bytes.len(), 84 + 50 * 2);
        let decoded = read_stl(ShortReadCursor::new(bytes), Tolerance::DEFAULT).unwrap();
        assert_eq!(decoded.triangles().len(), 2);
        assert_eq!(decoded.triangle_points(0), original.triangle_points(0));
    }

    #[test]
    fn ascii_round_trip_preserves_geometry() {
        let original = mesh();
        let mut bytes = Vec::new();
        write_stl(&mut bytes, &original, StlFormat::Ascii).unwrap();
        assert!(bytes.starts_with(b"solid viboceros\n"));
        let decoded = read_stl(Cursor::new(bytes), Tolerance::DEFAULT).unwrap();
        assert_eq!(decoded.triangles().len(), 2);
        assert_eq!(decoded.triangle_points(1), original.triangle_points(1));
    }

    #[test]
    fn reads_the_ascii_pyramid_fixture() {
        let bytes = include_bytes!("../tests/fixtures/pyramid_ascii.stl");
        let decoded = read_stl(Cursor::new(bytes), Tolerance::DEFAULT).unwrap();
        assert_eq!(decoded.triangles().len(), 6);
        assert_eq!(
            decoded.bounds().min(),
            Point3::try_new(-4.0, -4.0, 0.0).unwrap()
        );
        assert_eq!(
            decoded.bounds().max(),
            Point3::try_new(4.0, 4.0, 6.0).unwrap()
        );
    }

    #[test]
    fn rejects_malformed_ascii_and_binary_lengths() {
        let ascii = b"solid bad\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nendsolid bad\n";
        assert!(read_stl(Cursor::new(ascii), Tolerance::DEFAULT).is_err());

        let mut binary = vec![0_u8; 84];
        binary[80..84].copy_from_slice(&1_u32.to_le_bytes());
        assert!(matches!(
            read_stl(Cursor::new(binary), Tolerance::DEFAULT),
            Err(StlError::BinaryLengthMismatch { .. })
        ));

        let mut non_finite_normal = Vec::new();
        write_stl(&mut non_finite_normal, &mesh(), StlFormat::Binary).unwrap();
        non_finite_normal[84..88].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(matches!(
            read_stl(Cursor::new(non_finite_normal), Tolerance::DEFAULT),
            Err(StlError::NonFiniteBinaryValue {
                triangle: 0,
                field: "normal"
            })
        ));
    }

    #[test]
    fn binary_export_rejects_coordinates_outside_f32() {
        let huge = TriangleMesh::try_new(
            vec![
                Point3::try_new(f64::MAX, 0.0, 0.0).unwrap(),
                Point3::try_new(f64::MAX, 1.0, 0.0).unwrap(),
                Point3::try_new(f64::MAX, 0.0, 1.0).unwrap(),
            ],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(matches!(
            write_stl(Vec::new(), &huge, StlFormat::Binary),
            Err(StlError::F32OutOfRange { .. })
        ));
    }

    #[test]
    fn binary_export_rejects_faces_collapsed_by_f32_rounding_before_truncating_a_file() {
        let precision_loss = TriangleMesh::try_new(
            vec![
                Point3::try_new(1.0e10, 0.0, 0.0).unwrap(),
                Point3::try_new(1.0e10 + 1.0, 0.0, 0.0).unwrap(),
                Point3::try_new(1.0e10, 1.0, 0.0).unwrap(),
            ],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(matches!(
            write_stl(Vec::new(), &precision_loss, StlFormat::Binary),
            Err(StlError::BinaryPrecisionLoss { triangle: 0 })
        ));

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "viboceros-{}-{unique}-preserve.stl",
            std::process::id()
        ));
        fs::write(&path, b"existing data").unwrap();
        assert!(matches!(
            write_stl_file(&path, &precision_loss, StlFormat::Binary),
            Err(StlError::BinaryPrecisionLoss { triangle: 0 })
        ));
        assert_eq!(fs::read(&path).unwrap(), b"existing data");
        fs::remove_file(path).unwrap();
    }

    struct ShortReadCursor(Cursor<Vec<u8>>);

    impl ShortReadCursor {
        fn new(bytes: Vec<u8>) -> Self {
            Self(Cursor::new(bytes))
        }
    }

    impl Read for ShortReadCursor {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            let length = buffer.len().min(1);
            self.0.read(&mut buffer[..length])
        }
    }

    impl Seek for ShortReadCursor {
        fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
            self.0.seek(position)
        }
    }
}

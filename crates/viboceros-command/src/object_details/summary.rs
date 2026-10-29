//! Representation facts use the stored model, without fitting or tessellating.
use super::*;

pub(super) fn write(out: &mut Report, doc: &Document, geometry: &Geometry) {
    match geometry {
        Geometry::Point(point) => writeln!(out, "Point at {:?}", point.to_array()).unwrap(),
        Geometry::PointCloud(cloud) => {
            writeln!(
                out,
                "Point cloud: {} points ({} hidden)",
                cloud.points().len(),
                cloud.hidden_count()
            )
            .unwrap();
        }
        Geometry::Line(line) => {
            writeln!(
                out,
                "Line: {:?} to {:?}",
                line.start().to_array(),
                line.end().to_array()
            )
            .unwrap();
        }
        Geometry::Circle(circle) => {
            writeln!(
                out,
                "Circle: center {:?}, radius {}",
                circle.center().to_array(),
                circle.radius()
            )
            .unwrap();
        }
        Geometry::Arc(arc) => {
            let angles = arc.angle_domain();
            writeln!(
                out,
                "Arc: radius {}, angles [{}, {}] radians",
                arc.radius(),
                angles.start(),
                angles.end()
            )
            .unwrap();
        }
        Geometry::Ellipse(ellipse) => {
            writeln!(
                out,
                "Ellipse: radii {}, {}",
                ellipse.radius_x(),
                ellipse.radius_y()
            )
            .unwrap();
        }
        Geometry::Polyline(polyline) => {
            writeln!(out, "Polyline: {} vertices", polyline.vertices().len()).unwrap();
        }
        Geometry::PolyCurve(polycurve) => {
            writeln!(out, "Polycurve: {} segments", polycurve.segments().len()).unwrap();
        }
        Geometry::NurbsCurve(curve) => {
            writeln!(
                out,
                "NURBS curve: degree {}, {} control points; rational: {}",
                curve.degree(),
                curve.control_points().len(),
                curve.has_rational_representation()
            )
            .unwrap();
        }
        Geometry::NurbsSurface(surface) => {
            writeln!(
                out,
                "NURBS surface: degrees {}, {}",
                surface.degree_u(),
                surface.degree_v()
            )
            .unwrap();
            writeln!(
                out,
                "Control points: {} x {}; rational: {}",
                surface.control_point_count_u(),
                surface.control_point_count_v(),
                surface.is_rational()
            )
            .unwrap();
            let (u, v) = (surface.domain_u(), surface.domain_v());
            writeln!(
                out,
                "Domains: U [{}, {}], V [{}, {}]",
                u.start(),
                u.end(),
                v.start(),
                v.end()
            )
            .unwrap();
        }
        Geometry::Brep(brep) => {
            let name = if brep.is_extrusion() {
                "Extrusion"
            } else if brep.faces().len() > 1 {
                "Polysurface"
            } else {
                "Surface (B-rep)"
            };
            writeln!(
                out,
                "{}: {} faces, {} edges, {} vertices",
                name,
                brep.faces().len(),
                brep.edges().len(),
                brep.vertices().len()
            )
            .unwrap();
        }
        Geometry::Mesh(mesh) => {
            writeln!(
                out,
                "Mesh: {} vertices, {} faces",
                mesh.vertices().len(),
                mesh.faces().len()
            )
            .unwrap();
        }
        Geometry::EmptyMesh => writeln!(out, "Empty mesh: 0 vertices, 0 faces").unwrap(),
        Geometry::SubD(subd) => {
            writeln!(
                out,
                "SubD: {} control vertices, {} edges, {} faces",
                subd.control_points().len(),
                subd.edges().len(),
                subd.faces().len()
            )
            .unwrap();
        }
        Geometry::BlockInstance(instance) => {
            let reference = instance.reference();
            if let Some(definition) = doc.block_definition(reference.definition()) {
                writeln!(out, "Block instance: {:?}", definition.name()).unwrap();
            }
            writeln!(out, "Definition ID: {}", reference.definition()).unwrap();
        }
    }
    if let Some(curve) = geometry.curve_ref() {
        let domain = curve.domain();
        writeln!(out, "Domain: [{}, {}]", domain.start(), domain.end()).unwrap();
    }
}

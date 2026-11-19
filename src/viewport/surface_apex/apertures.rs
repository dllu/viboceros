//! Classify cutter apertures against the live projected section.
use super::*;

#[derive(Debug)]
pub(super) struct SectionApertures {
    holes: Vec<Vec<[Point3; 2]>>,
    plane: Option<Plane>,
}

type Segment = [[f64; 2]; 2];

impl SectionApertures {
    pub(super) fn capture(boundary: &Brep, tolerance: Tolerance) -> Option<Self> {
        let [face] = boundary.faces() else {
            return None;
        };
        let holes: Vec<_> = face
            .loops()
            .iter()
            .skip(1)
            .filter_map(|hole| {
                let mut edges = Vec::new();
                for edge in hole.trims().iter().filter_map(|trim| trim.edge()) {
                    boundary.edges()[edge]
                        .curve()
                        .visit_segments(|a, b| edges.push([a, b]));
                }
                (!edges.is_empty()).then_some(edges)
            })
            .collect();
        (!holes.is_empty()).then(|| Self {
            holes,
            plane: face.surface().plane(tolerance).ok().flatten(),
        })
    }

    pub(super) fn append(
        &self,
        section: &[[Point3; 2]],
        apex: Point3,
        source_plane: Option<Plane>,
        edges: &mut Vec<[Point3; 2]>,
    ) -> Option<()> {
        // View the rim and hole loops along the sweep rulings in a common
        // plane. This also works with a curved cutter and a planar source.
        let first = section.first()?[0];
        let normal = match self.plane.or(source_plane) {
            Some(plane) => plane.normal(),
            None => apex.direction_to(first).ok()?,
        };
        let frame = Frame3::try_from_unit_normal(apex, normal).ok()?;
        let depth = frame.coordinates_of(first).ok()?[2];
        let project = |point: Point3| {
            let [x, y, z] = frame.coordinates_of(point).ok()?;
            if depth == 0. || z == 0. || z.is_sign_positive() != depth.is_sign_positive() {
                return None;
            }
            let scale = depth / z;
            let uv = [x * scale, y * scale];
            uv.iter().all(|value| value.is_finite()).then_some(uv)
        };
        let project_edges = |edges: &[[Point3; 2]]| {
            edges
                .iter()
                .map(|&[a, b]| Some([project(a)?, project(b)?]))
                .collect::<Option<Vec<_>>>()
        };
        let Some(outline) = project_edges(section) else {
            // There is no common finite view plane for this outline.
            return Some(());
        };
        for hole in &self.holes {
            let Some(projected) = project_edges(hole) else {
                continue;
            };
            if projected
                .iter()
                .any(|edge| outline.iter().any(|rim| meets(*rim, *edge)))
            {
                return None;
            }
            // Count crossings of all section loops together, including
            // openings carried through from the source surface.
            if contains(&outline, projected[0][0]) {
                edges.extend_from_slice(hole);
            }
        }
        Some(())
    }
}

fn contains(outline: &[Segment], [x, y]: [f64; 2]) -> bool {
    outline.iter().fold(false, |inside, &[[ax, ay], [bx, by]]| {
        let crosses = (ay > y) != (by > y) && x < ax + (bx - ax) * ((y - ay) / (by - ay));
        inside ^ crosses
    })
}

fn meets(a: Segment, b: Segment) -> bool {
    let overlaps = |axis: usize| {
        a[0][axis].min(a[1][axis]) <= b[0][axis].max(b[1][axis])
            && b[0][axis].min(b[1][axis]) <= a[0][axis].max(a[1][axis])
    };
    if !overlaps(0) || !overlaps(1) {
        return false;
    }
    let side = |[[ax, ay], [bx, by]]: Segment, [x, y]: [f64; 2]| {
        (bx - ax).mul_add(y - ay, -(by - ay) * (x - ax))
    };
    let spans = |segment, start, end| {
        let first = side(segment, start);
        let last = side(segment, end);
        first == 0. || last == 0. || first.is_sign_positive() != last.is_sign_positive()
    };
    spans(a, b[0], b[1]) && spans(b, a[0], a[1])
}

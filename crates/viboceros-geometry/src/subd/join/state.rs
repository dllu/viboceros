use super::*;
type Edge = (u32, u32);
fn key(a: u32, b: u32) -> Edge {
    (a.min(b), a.max(b))
}
#[derive(Clone, Copy)]
struct Use {
    count: usize,
    forward: bool,
}
pub(super) struct Assembly<'a> {
    sources: &'a [&'a SubD],
    pub order: Vec<usize>,
    maps: HashMap<usize, Vec<u32>>,
    flips: Vec<bool>,
    points: Vec<Point3>,
    edges: HashMap<Edge, Use>,
    faces: HashSet<Vec<u32>>,
    boundary: Vec<usize>,
}
fn face_key(face: &[u32]) -> Vec<u32> {
    let start = face.iter().enumerate().min_by_key(|(_, v)| *v).unwrap().0;
    let forward = (0..face.len())
        .map(|i| face[(start + i) % face.len()])
        .collect::<Vec<_>>();
    let backward = (0..face.len())
        .map(|i| face[(start + face.len() - i) % face.len()])
        .collect::<Vec<_>>();
    forward.min(backward)
}
impl<'a> Assembly<'a> {
    pub fn new(sources: &'a [&'a SubD], seed: usize) -> Result<Self, GeometryError> {
        let source = sources[seed];
        let mut result = Self {
            sources,
            order: vec![seed],
            maps: HashMap::from([(seed, (0..source.control_points().len() as u32).collect())]),
            flips: vec![false],
            points: source.control_points().to_vec(),
            edges: HashMap::new(),
            faces: HashSet::new(),
            boundary: vec![0; source.control_points().len()],
        };
        for face in source.faces() {
            result.add_face(face);
        }
        Ok(result)
    }
    fn add_face(&mut self, face: &[u32]) {
        self.faces.insert(face_key(face));
        for (i, &a) in face.iter().enumerate() {
            let b = face[(i + 1) % face.len()];
            let k = key(a, b);
            let edge = self.edges.entry(k).or_insert(Use {
                count: 0,
                forward: a < b,
            });
            if edge.count == 0 {
                self.boundary[a as usize] += 1;
                self.boundary[b as usize] += 1;
            }
            if edge.count == 1 {
                self.boundary[a as usize] -= 1;
                self.boundary[b as usize] -= 1;
            }
            edge.count += 1;
        }
    }
    pub fn append(
        &mut self,
        next: usize,
        contacts: &Contacts,
        tolerance: f64,
        nonmanifold: bool,
    ) -> Result<Option<bool>, GeometryError> {
        contacts.charge_admission(next)?;
        let source = self.sources[next];
        let mut mapping = vec![u32::MAX; source.control_points().len()];
        let mut distances = vec![f64::INFINITY; mapping.len()];
        for contact in &contacts.rows[next] {
            let Some(other) = self.maps.get(&contact.other) else {
                continue;
            };
            let target = other[contact.other_vertex];
            if self.boundary[target as usize] == 0 {
                continue;
            }
            let distance = source.control_points()[contact.vertex]
                .distance_to(self.points[target as usize])?;
            let old = (distances[contact.vertex], mapping[contact.vertex]);
            if distance <= tolerance && (distance < old.0 || distance == old.0 && target < old.1) {
                mapping[contact.vertex] = target;
                distances[contact.vertex] = distance;
            }
        }
        if mapping.iter().all(|&v| v == u32::MAX) {
            return Ok(None);
        }
        let mut new_points = Vec::new();
        for (i, value) in mapping.iter_mut().enumerate() {
            if *value == u32::MAX {
                *value = (self.points.len() + new_points.len()) as u32;
                new_points.push(source.control_points()[i]);
            }
        }
        let mut faces = Vec::new();
        let mut flip = None;
        let mut residual = false;
        for face in source.faces() {
            let mapped = face
                .iter()
                .map(|&v| mapping[v as usize])
                .collect::<Vec<_>>();
            let fk = face_key(&mapped);
            if mapped.iter().collect::<HashSet<_>>().len() != mapped.len()
                || self.faces.contains(&fk)
            {
                return Ok(None);
            }
            for (i, &a) in mapped.iter().enumerate() {
                let b = mapped[(i + 1) % mapped.len()];
                if let Some(old) = self.edges.get(&key(a, b)) {
                    if old.count > 1 {
                        if !nonmanifold {
                            return Ok(None);
                        }
                        residual = true;
                    }
                    let reversed = old.forward == (a < b);
                    if flip.is_some_and(|f| f != reversed) {
                        return Ok(None);
                    }
                    flip = Some(reversed);
                }
            }
            faces.push(mapped);
        }
        let flip = flip.unwrap_or(false);
        self.points.extend(new_points);
        self.boundary.resize(self.points.len(), 0);
        for mut face in faces {
            if flip {
                face[1..].reverse();
            }
            self.add_face(&face);
        }
        self.order.push(next);
        self.flips.push(flip);
        self.maps.insert(next, mapping);
        Ok(Some(residual))
    }
    pub fn finish(self, crease: bool) -> Result<SubDJoinResult, GeometryError> {
        let inputs = self
            .order
            .iter()
            .zip(&self.flips)
            .map(|(&i, &flip)| (self.sources[i].archive(), self.maps[&i].as_slice(), flip))
            .collect::<Vec<_>>();
        let subd = SubD::from_native(viboceros_opennurbs::subd_join_assemble(&inputs, crease)?)?;
        Ok(SubDJoinResult {
            subd,
            source_indices: self.order,
        })
    }
}

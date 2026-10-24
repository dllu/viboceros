// Independent triangle imprints, original boundaries, and coplanar pairs.
#include <SmTree.h>
struct VbMeshIntersections {
    std::vector<double> transverse;
    std::vector<double> tangential;
    // Four integers per pair: first face, second face, and their boundary masks.
    std::vector<uint32_t> coplanar_pairs;
};

namespace vb_mesh_intersections_detail {
bool coincident(SmPolyFace &a, SmPolyFace &b, double tolerance) {
    SmBoolean same = FALSE;
    double distance = 0.;
    check(a.CoincidenceCheck(b, tolerance, same, distance), "classify intersection planes");
    return same;
}

void append_segment(std::vector<double> &out, SmPolyEdge &edge) {
    const auto &a = edge.GetStartPolyVertex()->GetPoint();
    const auto &b = edge.GetEndPolyVertex()->GetPoint();
    require(std::isfinite(a.x) && std::isfinite(a.y) && std::isfinite(a.z) &&
        std::isfinite(b.x) && std::isfinite(b.y) && std::isfinite(b.z),
        "Nonfinite polygon intersection segment");
    require(out.size() / 6 < limit, "Polygon intersection segment limit");
    out.insert(out.end(), {a.x, a.y, a.z, b.x, b.y, b.z});
}
}

#include "mesh_intersection_origins.h"

namespace vb_mesh_intersections_detail {
std::unique_ptr<SmPolyBrep> copy_face(SmPolyFace &face, double tolerance) {
    auto mesh = std::unique_ptr<SmPolyBrep>(new (*SmApiGetOrCreateContext()) SmPolyBrep(tolerance));
    SmTArray<SmPolyFace *> faces;
    faces.Add(&face);
    check(face.GetPolyBrep()->CopyFaces(faces, mesh.get()), "copy intersection triangle");
    return mesh;
}
void intersect_pair(SmPolyFace &first, SmPolyFace &second, double tolerance,
    const vb_mesh_intersection_origins::Boundaries &ab,
    const vb_mesh_intersection_origins::Boundaries &bb, VbMeshIntersections &result) {
    auto a = copy_face(first, tolerance), b = copy_face(second, tolerance);
    SmPolyIntersector query(*SmApiGetOrCreateContext(), a.get(), b.get(), tolerance, 1e-10);
    check(query.IntersectInsertRelate(), "intersect independent triangle pair");
    SmTArray<SmPolyEdge *> edges, other_edges;
        query.GetCommonEdges(edges, other_edges);
        require(edges.GetSize() <= limit, "Polygon intersection edge limit");
        size_t work = 0;
        for (ULONG i = 0; i < edges.GetSize(); ++i) {
            SmTArray<SmPolyEdge *> ar, br;
            edges[i]->GetAllRadials(ar);
            other_edges[i]->GetAllRadials(br);
            bool transverse = false;
            for (ULONG j = 0; j < ar.GetSize() && !transverse; ++j) {
                for (ULONG k = 0; k < br.GetSize() && !transverse; ++k) {
                    require(++work <= 10 * limit, "Intersection radial work limit");
                    require(ar[j]->GetPolyFace() && br[k]->GetPolyFace(), "Missing imprinted radial face");
                    transverse = !vb_mesh_intersections_detail::coincident(
                        *ar[j]->GetPolyFace(), *br[k]->GetPolyFace(), tolerance);
                }
            }
            if (transverse) {
                const bool contact = vb_mesh_intersection_origins::contains(first, *edges[i], ab, tolerance) &&
                    vb_mesh_intersection_origins::contains(second, *edges[i], bb, tolerance);
                vb_mesh_intersections_detail::append_segment(contact ? result.tangential : result.transverse, *edges[i]);
            }
        }
 }

// Query original triangles before imprinting changes their boundaries. A
// centroid of a resulting face cannot establish whole-region containment.
void compute_pairs(SmPolyBrep &a, const std::vector<SmPolyFace *> &af,
    const std::vector<SmPolyFace *> &bf, double tolerance,
    const vb_mesh_intersection_origins::Boundaries &ab,
    const vb_mesh_intersection_origins::Boundaries &bb,
    const std::vector<uint32_t> &am, const std::vector<uint32_t> &bm, VbMeshIntersections &out,
    bool self_intersections = false) {
    SmExtent3d bounds;
    check(a.CalculateBoundingBox(bounds), "bound intersection mesh");
    auto tree = std::unique_ptr<SmTree>(new (*SmApiGetOrCreateContext()) SmTree(bounds));
    std::unordered_map<SmPolyFace *, uint32_t> indices;
    for (size_t i = 0; i < af.size(); ++i) {
        SmExtent3d box;
        check(af[i]->CalculateBoundingBox(box), "bound intersection triangle");
        check(tree->AddToSpatialTree(box, af[i], SM_NG_POLYGON), "index intersection triangle");
        indices.emplace(af[i], static_cast<uint32_t>(i));
    }
    size_t work = 0;
    for (size_t j = 0; j < bf.size(); ++j) {
        SmExtent3d box;
        check(bf[j]->CalculateBoundingBox(box), "bound other intersection triangle");
        box.ExpandAbsolute(tolerance);
        SmTArray<SmObjectList *> candidates;
        check(tree->GetObjectListInBox(box, candidates), "query intersection triangles");
        std::vector<SmPolyFace *> ordered;
        ordered.reserve(candidates.GetSize());
        for (ULONG k = 0; k < candidates.GetSize(); ++k)
            ordered.push_back(static_cast<SmPolyFace *>(candidates[k]->m_pObject));
        if (self_intersections) std::sort(ordered.begin(), ordered.end(),
            [&](SmPolyFace *first, SmPolyFace *second) { return indices.at(first) < indices.at(second); });
        for (auto *face : ordered) {
            require(++work <= 10 * limit, "Mesh intersection candidate work limit");
            const auto i = indices.at(face);
            if (self_intersections && i >= j) continue;
            if (coincident(*face, *bf[j], tolerance)) {
                require(out.coplanar_pairs.size() / 4 < limit, "Coplanar triangle pair limit");
                out.coplanar_pairs.insert(out.coplanar_pairs.end(), {i, static_cast<uint32_t>(j), am[i], bm[j]});
            } else {
                // Two noncoplanar triangles sharing an entire edge meet only
                // along that ordinary adjacency, including unwelded vertices.
                bool adjacent = false;
                if (self_intersections) {
                    SmTArray<SmPolyEdge *> ae, be;
                    face->GetPolyEdges(ae); bf[j]->GetPolyEdges(be);
                    const auto equal = [](const SmPoint3d &p, const SmPoint3d &q) {
                        return p.x==q.x && p.y==q.y && p.z==q.z;
                    };
                    for (ULONG u = 0; u < ae.GetSize(); ++u) {
                        for (ULONG v = 0; v < be.GetSize(); ++v) {
                            adjacent |= (equal(ae[u]->GetStartPoint(), be[v]->GetStartPoint()) &&
                                equal(ae[u]->GetEndPoint(), be[v]->GetEndPoint())) ||
                                (equal(ae[u]->GetStartPoint(), be[v]->GetEndPoint()) &&
                                equal(ae[u]->GetEndPoint(), be[v]->GetStartPoint()));
                        }
                    }
                }
                if (!adjacent) intersect_pair(*face, *bf[j], tolerance, ab, bb, out);
            }
        }
    }
}

}

extern "C" int vb_mesh_intersections(const double *first, size_t first_count,
    const uint32_t *first_faces, size_t first_face_count, const double *second,
    size_t second_count, const uint32_t *second_faces, size_t second_face_count,
    double tolerance, VbMeshIntersections **out, char *error, size_t capacity) {
    if (out) *out = nullptr;
    return guarded(error, capacity, [&] {
        require(out && std::isfinite(tolerance) && tolerance > 0., "Invalid mesh intersection tolerance");
        std::vector<SmPolyFace *> af, bf;
        auto a = vb_mesh_boolean_detail::import_mesh(first, first_count, first_faces, first_face_count, tolerance, &af);
        auto b = vb_mesh_boolean_detail::import_mesh(second, second_count, second_faces, second_face_count, tolerance, &bf);
        auto result = std::make_unique<VbMeshIntersections>();
        const auto ab = vb_mesh_intersection_origins::collect(*a, tolerance);
        const auto bb = vb_mesh_intersection_origins::collect(*b, tolerance);
        const auto am = vb_mesh_intersection_origins::masks(af, first, first_faces, ab);
        const auto bm = vb_mesh_intersection_origins::masks(bf, second, second_faces, bb);
        vb_mesh_intersections_detail::compute_pairs(*a, af, bf, tolerance, ab, bb, am, bm, *result);
        *out = result.release();
    });
}

extern "C" int vb_mesh_self_intersections(const double *points, size_t point_count,
    const uint32_t *triangles, size_t face_count, double tolerance,
    VbMeshIntersections **out, char *error, size_t capacity) {
    if (out) *out = nullptr;
    return guarded(error, capacity, [&] {
        require(out && std::isfinite(tolerance) && tolerance > 0., "Invalid mesh self intersection tolerance");
        std::vector<SmPolyFace *> faces;
        auto mesh = vb_mesh_boolean_detail::import_mesh(points, point_count, triangles, face_count, tolerance, &faces);
        auto result = std::make_unique<VbMeshIntersections>();
        const auto boundaries = vb_mesh_intersection_origins::collect(*mesh, tolerance);
        const auto masks = vb_mesh_intersection_origins::masks(faces, points, triangles, boundaries);
        vb_mesh_intersections_detail::compute_pairs(*mesh, faces, faces, tolerance,
            boundaries, boundaries, masks, masks, *result, true);
        *out = result.release();
    });
}

extern "C" int vb_mesh_intersections_sizes(const VbMeshIntersections *p, size_t *a, size_t *t, size_t *b) {
    if (!p || !a || !t || !b) return -1;
    *a = p->transverse.size() / 6; *t = p->tangential.size() / 6; *b = p->coplanar_pairs.size() / 4; return 0;
}
extern "C" int vb_mesh_intersections_copy(const VbMeshIntersections *p, double *a, size_t na, double *t, size_t nt, uint32_t *b, size_t nb) {
    if (!p || na != p->transverse.size() / 6 || nt != p->tangential.size() / 6 || nb != p->coplanar_pairs.size() / 4 || (!a && na) || (!t && nt) || (!b && nb)) return -1;
    std::copy(p->transverse.begin(), p->transverse.end(), a);
    std::copy(p->tangential.begin(), p->tangential.end(), t);
    std::copy(p->coplanar_pairs.begin(), p->coplanar_pairs.end(), b); return 0;
}
extern "C" void vb_mesh_intersections_free(VbMeshIntersections *p) { delete p; }

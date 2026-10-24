// Intersection lines on owned imprints, with original coplanar triangle pairs.
#include <SmTree.h>
struct VbMeshIntersections {
    std::vector<double> transverse;
    std::vector<uint32_t> coplanar_pairs;
};

namespace vb_mesh_intersections_detail {
bool coincident(SmPolyFace &a, SmPolyFace &b, double tolerance) {
    SmBoolean same = FALSE;
    double distance = 0.;
    check(a.CoincidenceCheck(b, tolerance, same, distance), "classify intersection planes");
    return same;
}

// Query original triangles before imprinting changes their boundaries. A
// centroid of a resulting face cannot establish whole-region containment.
void coplanar_pairs(SmPolyBrep &a, const std::vector<SmPolyFace *> &af,
    const std::vector<SmPolyFace *> &bf, double tolerance, std::vector<uint32_t> &out) {
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
        for (ULONG k = 0; k < candidates.GetSize(); ++k) {
            require(++work <= 10 * limit, "Mesh intersection candidate work limit");
            auto *face = static_cast<SmPolyFace *>(candidates[k]->m_pObject);
            if (!coincident(*face, *bf[j], tolerance)) continue;
            require(out.size() / 2 < limit, "Coplanar triangle pair limit");
            out.insert(out.end(), {indices.at(face), static_cast<uint32_t>(j)});
        }
    }
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
        vb_mesh_intersections_detail::coplanar_pairs(*a, af, bf, tolerance, result->coplanar_pairs);
        SmPolyIntersector query(*SmApiGetOrCreateContext(), a.get(), b.get(), tolerance, 1e-10);
        check(query.IntersectInsertRelate(), "intersect polygon meshes");
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
                    transverse = !vb_mesh_intersections_detail::coincident(
                        *ar[j]->GetPolyFace(), *br[k]->GetPolyFace(), tolerance);
                }
            }
            if (transverse) vb_mesh_intersections_detail::append_segment(result->transverse, *edges[i]);
        }
        *out = result.release();
    });
}

extern "C" int vb_mesh_intersections_sizes(const VbMeshIntersections *p, size_t *a, size_t *b) {
    if (!p || !a || !b) return -1;
    *a = p->transverse.size() / 6; *b = p->coplanar_pairs.size() / 2; return 0;
}
extern "C" int vb_mesh_intersections_copy(const VbMeshIntersections *p, double *a, size_t na, uint32_t *b, size_t nb) {
    if (!p || na != p->transverse.size() / 6 || nb != p->coplanar_pairs.size() / 2 || (!a && na) || (!b && nb)) return -1;
    std::copy(p->transverse.begin(), p->transverse.end(), a);
    std::copy(p->coplanar_pairs.begin(), p->coplanar_pairs.end(), b); return 0;
}
extern "C" void vb_mesh_intersections_free(VbMeshIntersections *p) { delete p; }

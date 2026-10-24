// Polygon Boolean adapter. Inputs are copied into owned SMLib topology.
#include <SmApiPolygons.h>
#include <SmAttribute.h>
#include <array>
#include <map>

namespace vb_mesh_boolean_detail {
std::unique_ptr<SmPolyBrep> import_mesh(const double *points, size_t count,
    const uint32_t *triangles, size_t face_count, double tolerance,
    std::vector<SmPolyFace *> *face_order = nullptr) {
    require(points && triangles && count > 0 && count <= limit &&
        face_count > 0 && face_count <= limit, "Invalid polygon Boolean mesh sizes");
    const auto *context = SmApiGetOrCreateContext();
    auto mesh = std::unique_ptr<SmPolyBrep>(new (*context) SmPolyBrep(tolerance));
    std::map<std::array<double, 3>, SmPolyVertex *> unique;
    std::vector<SmPolyVertex *> vertices(count, nullptr);
    for (size_t i = 0; i < face_count; ++i) {
        SmTArray<SmPolyVertex *> corners;
        for (size_t j = 0; j < 3; ++j) {
            const auto index = triangles[3*i+j];
            require(index < count && finite3(points+3*index), "Invalid polygon Boolean vertex");
            auto *&vertex = vertices[index];
            if (!vertex) {
                const std::array<double, 3> p{points[3*index], points[3*index+1], points[3*index+2]};
                auto &shared = unique[p];
                if (!shared) shared = new (mesh.get()) SmPolyVertex(SmPoint3d(p[0], p[1], p[2]), tolerance);
                vertex = shared;
            }
            corners.Add(vertex);
        }
        SmPolyFace *face = nullptr;
        check(mesh->CreatePolyFace(nullptr, nullptr, corners, face), "import polygon Boolean face");
        require(face, "Missing polygon Boolean face");
        if (face_order) face_order->push_back(face);
    }
    return mesh;
}

// The kernel consumes its second operand during the operation. Inspect its
// surviving pointers on every exit, including errors after that deletion.
class OwnedMerge final : public SmPolyMerge {
public:
    OwnedMerge(SmPolyBrep *first, SmPolyBrep *second, double tolerance)
        : SmPolyMerge(*SmApiGetOrCreateContext(), first, second, tolerance, 20.*SM_PI/180.) {}
    ~OwnedMerge() override {
        delete m_vPI.GetBrep();
        delete m_vPI.GetOther();
    }
    bool has_interaction() const {
        SmTArray<SmPolyEdge *> first_edges, second_edges;
        m_vPI.GetCommonEdges(first_edges, second_edges);
        if (first_edges.GetSize()) return true;
        SmTArray<SmPolyVertex *> first_vertices, second_vertices;
        m_vPI.GetCommonVertices(first_vertices, second_vertices);
        if (first_vertices.GetSize()) return true;
        // A contained shell has no intersection edges. Classification marks
        // the faces that union would remove, without deleting either operand.
        for (auto *mesh : {m_vPI.GetBrep(), m_vPI.GetOther()}) {
            require(mesh, "Missing classified polygon Boolean operand");
            SmTArray<SmPolyFace *> faces;
            mesh->GetPolyFaces(faces);
            for (ULONG i = 0; i < faces.GetSize(); ++i)
                if (faces[i]->FindAttribute(SM_AI_BOOLEAN_DELETE)) return true;
        }
        return false;
    }
};

std::unique_ptr<VbMesh> export_mesh(SmPolyBrep &mesh) {
    SmTArray<SmPolyFace *> faces;
    mesh.GetPolyFaces(faces);
    require(faces.GetSize() <= limit, "Polygon Boolean output work limit");
    if (!faces.GetSize()) return nullptr;
    for (ULONG i = 0; i < faces.GetSize(); ++i) {
        SmTArray<SmPolyFace *> triangles;
        check(faces[i]->TriangulatePlanar(triangles), "triangulate polygon Boolean result");
    }
    faces.RemoveAll();
    mesh.GetPolyFaces(faces);
    SmTArray<SmPolyVertex *> vertices;
    mesh.GetPolyVertices(vertices);
    require(vertices.GetSize() <= limit && faces.GetSize() <= limit, "Polygon Boolean output size limit");
    auto out = std::make_unique<VbMesh>();
    std::unordered_map<const SmPolyVertex *, uint32_t> indices;
    for (ULONG i = 0; i < vertices.GetSize(); ++i) {
        const auto &point = vertices[i]->GetPoint();
        require(std::isfinite(point.x) && std::isfinite(point.y) && std::isfinite(point.z),
            "Nonfinite polygon Boolean result");
        indices.emplace(vertices[i], static_cast<uint32_t>(i));
        out->points.insert(out->points.end(), {point.x, point.y, point.z});
    }
    for (ULONG i = 0; i < faces.GetSize(); ++i) {
        auto *loop = faces[i]->GetOuterPolyLoop();
        require(loop, "Missing polygon Boolean output loop");
        SmTArray<SmPolyEdge *> edges;
        loop->GetPolyEdges(edges);
        require(edges.GetSize() == 3, "Nontriangular polygon Boolean output");
        for (ULONG j = 0; j < 3; ++j)
            out->triangles.push_back(indices.at(edges[j]->GetStartPolyVertex()));
    }
    return out;
}
} // namespace vb_mesh_boolean_detail

extern "C" int vb_mesh_boolean(const double *first, size_t first_count,
    const uint32_t *first_faces, size_t first_face_count, const double *second,
    size_t second_count, const uint32_t *second_faces, size_t second_face_count,
    int operation, double tolerance, VbMesh **out, char *error, size_t capacity) {
    if (out) *out = nullptr;
    return guarded(error, capacity, [&] {
        require(out && operation >= 0 && operation <= 2 && std::isfinite(tolerance) && tolerance > 0.,
            "Invalid polygon Boolean operation or tolerance");
        auto a = vb_mesh_boolean_detail::import_mesh(first, first_count, first_faces, first_face_count, tolerance);
        auto b = vb_mesh_boolean_detail::import_mesh(second, second_count, second_faces, second_face_count, tolerance);
        vb_mesh_boolean_detail::OwnedMerge owner(a.get(), b.get(), tolerance);
        a.release();
        b.release();
        SmPolyBrep *result = nullptr;
        const auto kind = operation == 0 ? SM_PBO_UNION : operation == 1 ? SM_PBO_INTERSECTION : SM_PBO_DIFFERENCE;
        check(owner.ManifoldBoolean(kind, result), "polygon Boolean");
        require(result, "Missing polygon Boolean result");
        *out = vb_mesh_boolean_detail::export_mesh(*result).release();
    });
}

extern "C" int vb_mesh_boolean_interaction(const double *first, size_t first_count,
    const uint32_t *first_faces, size_t first_face_count, const double *second,
    size_t second_count, const uint32_t *second_faces, size_t second_face_count,
    double tolerance, int *interacts, char *error, size_t capacity) {
    if (interacts) *interacts = 0;
    return guarded(error, capacity, [&] {
        require(interacts && std::isfinite(tolerance) && tolerance > 0.,
            "Invalid polygon Boolean interaction tolerance");
        auto a = vb_mesh_boolean_detail::import_mesh(first, first_count, first_faces, first_face_count, tolerance);
        auto b = vb_mesh_boolean_detail::import_mesh(second, second_count, second_faces, second_face_count, tolerance);
        vb_mesh_boolean_detail::OwnedMerge owner(a.get(), b.get(), tolerance);
        a.release();
        b.release();
        owner.SetImprintAndClassify(TRUE);
        SmPolyBrep *result = nullptr;
        check(owner.ManifoldBoolean(SM_PBO_UNION, result), "classify polygon Boolean interaction");
        *interacts = owner.has_interaction() ? 1 : 0;
    });
}

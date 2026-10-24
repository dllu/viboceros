// Physical boundaries of the original inputs, independent of working imprints.
#include <unordered_set>
namespace vb_mesh_intersection_origins {
using Boundaries = std::unordered_set<const SmPolyEdge *>;

double side(SmPolyEdge &edge, SmPolyFace &face, const SmVector3d &normal) {
    const auto a = edge.GetStartPoint(), b = edge.GetEndPoint(), c = face.GetCentroid();
    const double x = b.x-a.x, y = b.y-a.y, z = b.z-a.z;
    const double u = c.x-a.x, v = c.y-a.y, w = c.z-a.z;
    const double value = (y*w-z*v)*normal.x + (z*u-x*w)*normal.y + (x*v-y*u)*normal.z;
    require(std::isfinite(value), "Nonfinite mesh boundary classification");
    return value;
}

Boundaries collect(SmPolyBrep &mesh, double tolerance) {
    SmTArray<SmPolyEdge *> edges;
    mesh.GetPolyEdges(edges);
    std::unordered_set<SmPolyEdge *> visited;
    Boundaries boundaries;
    size_t work = 0;
    for (ULONG i = 0; i < edges.GetSize(); ++i) {
        if (visited.count(edges[i])) continue;
        SmTArray<SmPolyEdge *> radials;
        edges[i]->GetAllRadials(radials);
        bool interior = false;
        for (ULONG j = 0; j < radials.GetSize(); ++j) visited.insert(radials[j]);
        for (ULONG j = 0; j < radials.GetSize() && !interior; ++j) {
            auto *a = radials[j]->GetPolyFace();
            require(a, "Missing original mesh face");
            const auto normal = a->GetNormal(TRUE, TRUE);
            const double first = side(*edges[i], *a, normal);
            for (ULONG k = j+1; k < radials.GetSize() && !interior; ++k) {
                require(++work <= 10*limit, "Mesh boundary radial work limit");
                auto *b = radials[k]->GetPolyFace();
                require(b, "Missing original radial face");
                if (!vb_mesh_intersections_detail::coincident(*a, *b, tolerance)) continue;
                const double second = side(*edges[i], *b, normal);
                interior = (first < 0. && second > 0.) || (first > 0. && second < 0.);
            }
        }
        if (interior) continue;
        for (ULONG j = 0; j < radials.GetSize(); ++j) boundaries.insert(radials[j]);
    }
    return boundaries;
}

bool contains(SmPolyFace &source, SmPolyEdge &line, const Boundaries &boundaries, double tolerance) {
    SmTArray<SmPolyEdge *> edges;
    source.GetPolyEdges(edges);
    for (ULONG i = 0; i < edges.GetSize(); ++i) {
        if (!boundaries.count(edges[i])) continue;
        double parameter = 0., first = 0., second = 0.;
        check(edges[i]->DropPoint(line.GetStartPoint(), parameter, first), "classify first boundary point");
        check(edges[i]->DropPoint(line.GetEndPoint(), parameter, second), "classify second boundary point");
        if (first <= tolerance && second <= tolerance) return true;
    }
    return false;
}

std::vector<uint32_t> masks(const std::vector<SmPolyFace *> &faces, const double *points,
    const uint32_t *triangles, const Boundaries &boundaries) {
    std::vector<uint32_t> out(faces.size(), 0);
    const auto equal = [](const SmPoint3d &p, const double *q) { return p.x==q[0] && p.y==q[1] && p.z==q[2]; };
    for (size_t i = 0; i < faces.size(); ++i) {
        SmTArray<SmPolyEdge *> edges;
        faces[i]->GetPolyEdges(edges);
        for (ULONG j = 0; j < edges.GetSize(); ++j) {
            if (!boundaries.count(edges[j])) continue;
            const auto a = edges[j]->GetStartPoint(), b = edges[j]->GetEndPoint();
            for (size_t k = 0; k < 3; ++k) {
                const double *p = points+3*triangles[3*i+k];
                const double *q = points+3*triangles[3*i+(k+1)%3];
                if ((equal(a,p) && equal(b,q)) || (equal(a,q) && equal(b,p))) out[i] |= 1U<<k;
            }
        }
    }
    return out;
}
}

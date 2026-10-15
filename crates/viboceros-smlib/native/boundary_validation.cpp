// Classify complete face-intersection output against the existing boundary graph.
#include "brep_data.h"
#include <SmApiBrep.h>
#include <SmApiIntersectors.h>
#include <SmApiQueries.h>
#include <SmBrep.h>
#include <SmBSplineSurface.h>
#include <SmCurve.h>
#include <SmCurveClass.h>
#include <SmEdge.h>
#include <SmFace.h>
#include <SmSolutionArray.h>
#include <SmVertex.h>
#include <algorithm>
#include <cmath>
#include <stdexcept>
#include <string>
#include <vector>
#include <memory>

namespace {
void require(bool value, const char *message) {
    if (!value) throw std::runtime_error(message);
}
void check(SmStatus status, const char *message) {
    if (status != SM_SUCCESS)
        throw std::runtime_error(std::string(message)+" status "+std::to_string(status));
}
struct CurveList {
    SmTArray<SmCurve *> values;
    ~CurveList() { for (ULONG i=0;i<values.GetSize();++i) delete values[i]; }
};
struct FaceBoundary {
    SmFace *face;
    SmTArray<SmEdge *> edges;
    SmTArray<SmVertex *> vertices;
};

bool same_support(SmSurface &first,SmSurface &second,double tolerance) {
    if(&first==&second) return true;
    // The upstream partial-coincidence heuristic can classify matching surface
    // boundaries as coincident despite an interior crossing. Only authorize its
    // shortcut for identical NURBS data or coplanar control-hull support.
    if(first.IsKindOf(SmBSplineSurface_TYPE)&&second.IsKindOf(SmBSplineSurface_TYPE)) {
        auto &a=static_cast<SmBSplineSurface &>(first);auto &b=static_cast<SmBSplineSurface &>(second);
        SmTArray<SmPoint3d> pa,pb;SmTArray<double> wa,wb;
        ULONG au=0,av=0,bu=0,bv=0;
        check(a.GetControlPointNet(au,av,pa,wa),"read first surface support");
        check(b.GetControlPointNet(bu,bv,pb,wb),"read second surface support");
        bool equal=au==bu&&av==bv&&pa.GetSize()==pb.GetSize()&&wa.GetSize()==wb.GetSize();
        for(ULONG i=0;equal&&i<pa.GetSize();++i)
            equal=pa[i].x==pb[i].x&&pa[i].y==pb[i].y&&pa[i].z==pb[i].z;
        for(ULONG i=0;equal&&i<wa.GetSize();++i)equal=wa[i]==wb[i];
        for(const auto direction:{SM_SP_U,SM_SP_V}) {
            if(!equal)break;
            SmTArray<double> ka,kb;SmTArray<ULONG> ma,mb;
            check(a.GetKnots(direction,ka,&ma),"read first surface knots");
            check(b.GetKnots(direction,kb,&mb),"read second surface knots");
            equal=a.GetDegree(direction)==b.GetDegree(direction)&&ka.GetSize()==kb.GetSize()&&ma.GetSize()==mb.GetSize();
            for(ULONG i=0;equal&&i<ka.GetSize();++i)equal=ka[i]==kb[i]&&ma[i]==mb[i];
        }
        if(equal)return true;
    }
    SmPoint3d a,b;SmVector3d na,nb;double da=0,db=0;
    if(!first.IsPlanar(tolerance/16,&a,&na,&da)||!second.IsPlanar(tolerance/16,&b,&nb,&db))return false;
    const double cx=na.y*nb.z-na.z*nb.y,cy=na.z*nb.x-na.x*nb.z,cz=na.x*nb.y-na.y*nb.x;
    if(cx!=0||cy!=0||cz!=0)return false;
    const double offset=std::abs(na.x*(b.x-a.x)+na.y*(b.y-a.y)+na.z*(b.z-a.z));
    return std::isfinite(offset)&&offset+da+db<=tolerance;
}

bool covered(SmCurve &curve, const std::vector<SmEdge *> &shared, double tolerance,
             size_t &comparisons, size_t &intervals) {
    const auto domain=curve.GetNaturalInterval();
    const double lo=domain.GetMin(), hi=domain.GetMax();
    require(std::isfinite(lo) && std::isfinite(hi) && lo<hi,
            "Invalid boundary intersection curve interval");
    std::vector<std::pair<double,double>> ranges;
    for (const auto edge:shared) {
        require(++comparisons<=65536, "Boundary coincidence work limit");
        const double edge_tolerance=edge->GetTolerance();
        require(std::isfinite(edge_tolerance)&&edge_tolerance>=0,"Invalid shared-edge tolerance");
        const double allowed=std::max(tolerance,edge_tolerance);
        SmSolutionArray solutions;
        SmBoolean more=FALSE;
        check(curve.GlobalCoincidenceChecker(domain,*edge->GetCurve(),edge->GetInterval(),
              allowed,more,solutions,FALSE), "classify boundary curve coincidence");
        require(solutions.GetSize()<=262144-intervals, "Boundary coincidence interval limit");
        intervals+=solutions.GetSize();
        for (ULONG i=0;i<solutions.GetSize();++i) {
            const auto &solution=solutions[i];
            if (solution.m_eSolutionType!=SM_ST_RANGE_OF_VALUES) continue;
            SmExtent1d first, second;
            check(solution.GetParameterInterval(0,first),"read intersection coincidence interval");
            check(solution.GetParameterInterval(1,second),"read edge coincidence interval");
            const auto edge_domain=edge->GetInterval();
            const double a=first.GetMin(), b=first.GetMax();
            require(std::isfinite(a)&&std::isfinite(b)&&a<=b &&
                    std::isfinite(second.GetMin())&&std::isfinite(second.GetMax()),
                    "Nonfinite boundary coincidence output");
            require(second.GetMin()>=edge_domain.GetMin() && second.GetMax()<=edge_domain.GetMax(),
                    "Boundary coincidence leaves the trimmed shared edge");
            if (b>=lo && a<=hi) ranges.emplace_back(std::max(a,lo),std::min(b,hi));
        }
    }
    std::sort(ranges.begin(),ranges.end());
    double cursor=lo;
    for (const auto &range:ranges) {
        if (range.first>cursor) return false;
        cursor=std::max(cursor,range.second);
        if (cursor>=hi) return true;
    }
    return false;
}

void interior_intersections(SmBrep &solid,SmFace &first,SmFace &second,double tolerance,
                            CurveList &output,SmTArray<SmPoint3d> &points,int quadrant) {
    auto domain=first.GetUVDomain();
    const auto lo=domain.GetMin(),hi=domain.GetMax();
    check(domain.SetMinMax(SmPoint2d(lo.x+(hi.x-lo.x)*((quadrant&1)?0.25:0.),lo.y+(hi.y-lo.y)*((quadrant&2)?0.25:0.)),
                          SmPoint2d(lo.x+(hi.x-lo.x)*((quadrant&1)?1.:0.75),lo.y+(hi.y-lo.y)*((quadrant&2)?1.:0.75))),
          "create interior boundary search domain");
    SmBoolean use_edges[2]={TRUE,TRUE};double angle=0.01;SmApproxTol3d approximation=tolerance*0.01;
    CurveList curves,uv_first,uv_second;
    SmTArray<SmTsectCurveType> types;
    check(first.GetSurface()->GlobalSurfaceIntersect(*solid.GetContext(),domain,
          *second.GetSurface(),second.GetUVDomain(),use_edges,&approximation,&angle,
          &curves.values,&uv_first.values,&uv_second.values,&types,nullptr),
          "search boundary surface interiors");
    require(curves.values.GetSize()<=65536&&curves.values.GetSize()==uv_first.values.GetSize()&&
            curves.values.GetSize()==uv_second.values.GetSize()&&curves.values.GetSize()==types.GetSize(),
            "Invalid interior intersection output");
    for(ULONG i=0;i<curves.values.GetSize();++i) {
        if(types[i]==SM_TC_TOUCHING) {
            // The main face query handles isolated contacts. Interior curve
            // searches supplement its missing transverse curve branches.
            continue;
        }
        auto *curve=curves.values[i];
        const auto interval=curve->GetNaturalInterval();
        auto a=std::unique_ptr<SmCurveClassification>(new (*solid.GetContext())
            SmCurveClassification(curve,interval,uv_first.values[i],tolerance));
        auto b=std::unique_ptr<SmCurveClassification>(new (*solid.GetContext())
            SmCurveClassification(curve,interval,uv_second.values[i],tolerance));
        check(first.CurveOnClassify(TRUE,*a),"clip interior intersection to first face");
        check(second.CurveOnClassify(TRUE,*b),"clip interior intersection to second face");
        CurveList clipped;
        SmTArray<SmPoint3d> contacts;
        check(a->ExtractGeometry(*solid.GetContext(),*b,clipped.values,nullptr,nullptr,&contacts),
              "extract trimmed interior intersections");
        for(ULONG c=0;c<clipped.values.GetSize();++c) {
            output.values.Add(clipped.values[c]);clipped.values[c]=nullptr;
        }
        points.Append(contacts);
    }
}
}

void vb_validate_boundary_intersections(SmBrep &solid, double tolerance, bool allow_sheets) {
    require(std::isfinite(tolerance)&&tolerance>0,"Invalid boundary intersection tolerance");
    if (!solid.IsManifoldSolid()) {
        require(allow_sheets,"Boundary validation requires a manifold solid");
        SmTArray<SmEdge*> edges;solid.GetEdges(edges);
        for (ULONG i=0;i<edges.GetSize();++i)
            require(edges[i]->IsLamina() || edges[i]->IsManifold(),"Boundary validation rejects nonmanifold sheet edges");
        SmTArray<SmVertex*> isolated;solid.GetShellVertices(isolated);
        require(isolated.GetSize()==0,"Boundary validation rejects isolated shell vertices");
    }
    SmTArray<SmFace *> faces;
    solid.GetFaces(faces);
    require(faces.GetSize()<=362,"Boundary intersection work limit");
    std::vector<FaceBoundary> boundaries(faces.GetSize());
    for (ULONG i=0;i<faces.GetSize();++i) {
        boundaries[i].face=faces[i];
        faces[i]->GetEdges(boundaries[i].edges);
        faces[i]->GetVertices(boundaries[i].vertices);
    }
    size_t comparisons=0, intervals=0;
    for (size_t a=0;a<boundaries.size();++a) for(size_t b=a+1;b<boundaries.size();++b) {
        const auto &first=boundaries[a];const auto &second=boundaries[b];
        std::vector<SmEdge *> shared_edges;
        std::vector<SmVertex *> shared_vertices;
        for (ULONG i=0;i<first.edges.GetSize();++i)
            for(ULONG j=0;j<second.edges.GetSize();++j)
                if(first.edges[i]==second.edges[j]) shared_edges.push_back(first.edges[i]);
        for (ULONG i=0;i<first.vertices.GetSize();++i)
            for(ULONG j=0;j<second.vertices.GetSize();++j)
                if(first.vertices[i]==second.vertices[j]) shared_vertices.push_back(first.vertices[i]);
        CurveList curves;
        SmTArray<SmPoint3d> points;
        SmBoolean coincident=same_support(*first.face->GetSurface(),*second.face->GetSurface(),tolerance);
        check(first.face->FaceIntersect(*solid.GetContext(),second.face,tolerance*0.01,20.0,
              curves.values,nullptr,nullptr,&points,nullptr,nullptr,nullptr,nullptr,nullptr,&coincident),
              "check boundary face intersections");
        if(!shared_edges.empty()&&!coincident) {
            for(int quadrant=0;quadrant<4;++quadrant) {
                interior_intersections(solid,*first.face,*second.face,tolerance,curves,points,quadrant);
                interior_intersections(solid,*second.face,*first.face,tolerance,curves,points,quadrant);
            }
        }
        require(curves.values.GetSize()<=65536 && points.GetSize()<=262144,
                "Boundary intersection output limit");
        for (ULONG c=0;c<curves.values.GetSize();++c)
            require(covered(*curves.values[c],shared_edges,tolerance,comparisons,intervals),
                    "Boundary face intersection leaves shared topological edges");
        for (ULONG p=0;p<points.GetSize();++p) {
            const auto point=points[p];
            require(std::isfinite(point.x)&&std::isfinite(point.y)&&std::isfinite(point.z),
                    "Nonfinite boundary contact");
            bool shared=false;
            for(const auto vertex:shared_vertices) {
                const auto q=vertex->GetPoint();
                const double vertex_tolerance=vertex->GetTolerance();
                require(std::isfinite(vertex_tolerance)&&vertex_tolerance>=0,"Invalid shared-vertex tolerance");
                shared|=std::hypot(point.x-q.x,point.y-q.y,point.z-q.z)<=std::max(tolerance,vertex_tolerance);
            }
            for(const auto edge:shared_edges) {
                if(shared) break;
                SmPoint3d closest;double parameter=0,distance=0;
                check(SmApiEdgeClosestPoint(edge,point,closest,parameter,distance),
                      "classify shared-edge contact");
                require(std::isfinite(distance)&&distance>=0,"Invalid shared-edge contact distance");
                const double edge_tolerance=edge->GetTolerance();
                require(std::isfinite(edge_tolerance)&&edge_tolerance>=0,"Invalid shared-edge tolerance");
                shared=distance<=std::max(tolerance,edge_tolerance);
            }
            require(shared,"Boundary has non-topological face contacts");
        }
    }
}

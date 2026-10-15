// Reproduce surface planarity independently of the inflated topology zone.
#include <SmApiGeneral.h>
#include <SmApiSurfaces.h>
#include <SmSurface.h>
#include <SmTol.h>
#include <cmath>
#include <iostream>
#include <memory>
int main() {
    SmApiCreateContext();
    for(int warped=0;warped<2;++warped) {
        SmSurface *raw=nullptr;
        if(SmApiCreateSurfaceFromCornerPoints(SmPoint3d(0,0,0),SmPoint3d(4,0,4),
              SmPoint3d(0,3,0),SmPoint3d(4,3,4+warped*0.01),raw)!=SM_SUCCESS || !raw)return 2;
        std::unique_ptr<SmSurface> surface(raw);
        const double zone=SmTol::GetZoneTol3d(surface.get());
        const double tolerance=zone/16.;
        SmPoint3d origin;SmVector3d normal;double gap=0;
        const bool planar=surface->IsPlanar(tolerance,&origin,&normal,&gap);
        const bool correct=planar==!warped && std::isfinite(gap) &&
            (!planar || std::abs(std::abs(normal.Dot(SmVector3d(-1/std::sqrt(2.),0,1/std::sqrt(2.))))-1)<1e-12);
        std::cout<<"{\"warped\":"<<warped<<",\"planar\":"<<(planar?"true":"false")
                 <<",\"zone\":"<<zone<<",\"tolerance\":"<<tolerance<<",\"gap\":"<<gap<<",\"qualified\":"<<(correct?"true":"false")<<"}\n";
    }
}

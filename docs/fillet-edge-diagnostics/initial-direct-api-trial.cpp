
#include <SmApiPrimitives.h>
#include <SmApiBrep.h>
#include <SmApiGeneral.h>
#include <SmApiQueries.h>
#include <SmBrep.h>
#include <SmFace.h>
#include <SmEdge.h>
#include <SmBSplineSurface.h>
#include <SmBSplineCurve.h>
#include <memory>
#include <iostream>
int main(){for(int count:{1,3,12}){
 SmBrep *raw=nullptr;auto status=SmApiCreateBox(SmVector3d(0,0,0),10,10,10,raw);
 std::unique_ptr<SmBrep> body(raw);if(status!=SM_SUCCESS||!raw)return 1;
 SmTArray<SmEdge*> edges,chosen;body->GetEdges(edges);
 for(int i=0;i<count;i++)chosen.Add(edges[i]);
 status=SmApiFilletEdges(body.get(),chosen,1.0,1,1,1.0);
 std::cout<<"edges "<<count<<" status "<<status<<" manifold "<<body->IsManifoldSolid();
 double volume=0;auto vstatus=SmApiBrepComputeVolume(body.get(),1e-8,volume);
 SmTArray<SmFace*> faces;body->GetFaces(faces);body->GetEdges(edges);
 unsigned ns=0,nc=0;for(ULONG i=0;i<faces.GetSize();i++)ns+=faces[i]->GetSurface()->IsKindOf(SmBSplineSurface_TYPE);
 for(ULONG i=0;i<edges.GetSize();i++)nc+=edges[i]->GetCurve()->IsKindOf(SmBSplineCurve_TYPE);
 std::cout<<" volume "<<volume<<" vstatus "<<vstatus<<" faces "<<faces.GetSize()<<" exact_surfaces "<<ns<<" curves "<<edges.GetSize()<<" exact_curves "<<nc<<std::endl;
 }return 0;}

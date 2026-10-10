#include "bridge.h"
#include <cstdio>
#include <cstdlib>
void check(int status,const char* error){if(status){std::fprintf(stderr,"%s\n",error);std::exit(1);}}
int main(){
 for(int i=0;i<16;++i){
  char error[512]={};double origin[3]={0,0,0},size[3]={10,10,10};VbSolid* box=nullptr;check(vb_solid_primitive(0,origin,size,&box,error,sizeof(error)),error);
  VbBrep* snapshot=nullptr;check(vb_solid_brep(box,&snapshot,error,sizeof(error)),error);VbBrepView view{};check(vb_brep_view(snapshot,&view),error);
  size_t components[128]={};int inward[1]={0};VbSolid* imported=nullptr;check(vb_solid_from_brep(&view,1e-9,components,1,inward,&imported,error,sizeof(error)),error);
  double center[3]={5,5,-1},cylinder_size[3]={2,12,0};VbSolid* cutter=nullptr;check(vb_solid_primitive(2,center,cylinder_size,&cutter,error,sizeof(error)),error);
  VbSolid* result=nullptr;check(vb_solid_boolean(imported,cutter,2,&result,error,sizeof(error)),error);VbParts* parts=nullptr;check(vb_solid_parts(result,&parts,error,sizeof(error)),error);
  for(size_t n=0;n<vb_parts_count(parts);++n)vb_solid_free(vb_parts_take(parts,n));vb_parts_free(parts);
  vb_solid_free(result);vb_solid_free(cutter);vb_solid_free(imported);vb_brep_free(snapshot);vb_solid_free(box);
 }
 std::puts("16 import/boolean/extraction lifecycle cycles passed");
}

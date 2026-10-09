// Original public OpenNURBS placement and traversal stress fixtures.
#define main original_block_fixture_main
#include "generate.cpp"
#undef main

int main(int argc,char** argv){
  if(argc!=3)return 2;ON::Begin();const std::string source=argv[1],directory=argv[2];
  const char* names[]={"reflected_blocks.3dm","projective_block.3dm","singular_block.3dm"};
  for(int variant=0;variant<3;++variant){
    ONX_Model model;if(!model.Read(source.c_str(),nullptr))return 3;
    const auto& geometry=model.ModelGeometryComponentFromId(uuid(31));
    auto* instance=const_cast<ON_InstanceRef*>(ON_InstanceRef::Cast(geometry.Geometry(nullptr)));if(instance==nullptr)return 4;
    if(variant==0){ON_Xform mirror=ON_Xform::IdentityTransformation;mirror[0][0]=-1;instance->m_xform=mirror*instance->m_xform;instance->m_bbox.Transform(mirror);}
    if(variant==1)instance->m_xform[3][0]=0.125;
    if(variant==2){for(int column=0;column<4;++column)instance->m_xform[0][column]=0;}
    if(!model.Write((directory+"/"+names[variant]).c_str(),80,nullptr))return 5;
  }
  // An acyclic DAG with no leaf geometry has exponential traversal work.
  ONX_Model model;ON_Layer layer;layer.SetName(L"Empty blocks");model.AddModelComponent(layer,true);
  definition(model,900,L"Empty",{});
  for(unsigned level=1;level<=20;++level){
    add(model,reference(900+level-1,ON_Xform::IdentityTransformation),1000+2*level,true,L"left");
    add(model,reference(900+level-1,ON_Xform::IdentityTransformation),1001+2*level,true,L"right");
    const ON_wString name=ON_wString::FormatToString(L"Level%u",level);
    definition(model,900+level,name.Array(),{1000+2*level,1001+2*level});
  }
  add(model,reference(920,ON_Xform::IdentityTransformation),2000,false,L"branching");
  if(!model.Write((directory+"/branching_empty_blocks.3dm").c_str(),80,nullptr))return 6;
  ON::End();return 0;
}

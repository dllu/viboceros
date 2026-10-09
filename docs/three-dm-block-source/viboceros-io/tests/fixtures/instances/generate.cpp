// Original OpenNURBS fixture producer; no proprietary Rhino implementation.
#include "opennurbs_public.h"
#include <string>
#include <stdexcept>

ON_UUID uuid(unsigned n){ON_UUID id=ON_nil_uuid;id.Data1=n;return id;}
ON_ModelComponentReference add(ONX_Model& model,ON_Geometry* geometry,unsigned id,bool definition,const wchar_t* name){
  auto* attributes=new ON_3dmObjectAttributes();attributes->m_uuid=uuid(id);attributes->m_layer_index=0;
  attributes->SetName(name,true);if(definition)attributes->SetMode(ON::idef_object);
  attributes->SetColorSource(ON::color_from_parent);
  const auto result=model.AddManagedModelGeometryComponent(geometry,attributes,false);
  if(result.IsEmpty())throw std::runtime_error("geometry insertion failed");return result;
}
void definition(ONX_Model& model,unsigned id,const wchar_t* name,std::initializer_list<unsigned> members){
  ON_InstanceDefinition def;def.SetId(uuid(id));def.SetName(name);
  def.SetInstanceDefinitionType(ON_InstanceDefinition::IDEF_UPDATE_TYPE::Static);
  def.SetBoundingBox(id==101?ON_BoundingBox(ON_3dPoint(0,0,0),ON_3dPoint(1,2,3)):
      id==102?ON_BoundingBox(ON_3dPoint(1,2,3),ON_3dPoint(3,8,15)):
      ON_BoundingBox(ON_3dPoint(-1,-1,-1),ON_3dPoint(1,1,1)));
  ON_SimpleArray<ON_UUID> ids;for(unsigned member:members)ids.Append(uuid(member));def.SetInstanceGeometryIdList(ids);
  if(model.AddModelComponent(def,false).IsEmpty())throw std::runtime_error("definition insertion failed");
}
ON_InstanceRef* reference(unsigned definition,const ON_Xform& transform){
  auto* instance=new ON_InstanceRef();instance->m_instance_definition_uuid=uuid(definition);instance->m_xform=transform;
  instance->m_bbox=definition==101?ON_BoundingBox(ON_3dPoint(0,0,0),ON_3dPoint(1,2,3)):
      definition==102?ON_BoundingBox(ON_3dPoint(1,2,3),ON_3dPoint(3,8,15)):
      ON_BoundingBox(ON_3dPoint(-1,-1,-1),ON_3dPoint(1,1,1));
  instance->m_bbox.Transform(transform);return instance;
}
int main(int argc,char** argv){
  if(argc!=2)return 2;ON::Begin();const std::string directory=argv[1];
  for(int variant=0;variant<4;++variant){
    ONX_Model model;model.m_settings.m_ModelUnitsAndTolerances.m_unit_system=ON::LengthUnitSystem::Millimeters;
    ON_Layer layer;layer.SetName(L"Block geometry");layer.SetColor(ON_Color(5,10,15));model.AddModelComponent(layer,true);
    ON_Group group;group.SetIndex(0);group.SetName(L"Placed");model.AddModelComponent(group,true);
    add(model,new ON_Point(ON_3dPoint(1,2,3)),11,true,L"point");
    add(model,new ON_LineCurve(ON_3dPoint(0,0,0),ON_3dPoint(1,1,0)),12,true,L"line");
    auto* cloud=new ON_PointCloud();cloud->m_P.Append(ON_3dPoint(0,0,0));cloud->m_P.Append(ON_3dPoint(1,2,0));
    cloud->m_C.Append(ON_Color(12,34,56));cloud->m_C.Append(ON_Color(65,43,21));
    cloud->m_N.Append(ON_3dVector(0,0,1));cloud->m_N.Append(ON_3dVector(0,1,0));
    add(model,cloud,13,true,L"cloud");
    const ON_3dPoint corners[8]={ON_3dPoint(0,0,0),ON_3dPoint(1,0,0),ON_3dPoint(1,1,0),ON_3dPoint(0,1,0),ON_3dPoint(0,0,1),ON_3dPoint(1,0,1),ON_3dPoint(1,1,1),ON_3dPoint(0,1,1)};
    add(model,ON_BrepBox(corners,nullptr),14,true,L"box");definition(model,101,L"Prototype",{11,12,13,14});
    ON_Xform scale=ON_Xform::IdentityTransformation;scale[0][0]=2;scale[1][1]=3;scale[2][2]=4;scale[0][3]=1;scale[1][3]=2;scale[2][3]=3;
    add(model,reference(101,scale),21,true,L"nested");definition(model,102,L"Nested",{21});
    ON_Xform rotate=ON_Xform::IdentityTransformation;rotate[0][0]=0;rotate[0][1]=-1;rotate[1][0]=1;rotate[1][1]=0;rotate[0][3]=10;rotate[1][3]=20;rotate[2][3]=30;
    auto root=add(model,reference(variant==2?999:102,rotate),31,false,L"placed");
    auto* attrs=const_cast<ON_3dmObjectAttributes*>(ON_ModelGeometryComponent::FromModelComponentRef(root,nullptr)->Attributes(nullptr));
    attrs->SetColorSource(ON::color_from_object);attrs->m_color=ON_Color(100,110,120);attrs->AddToGroup(0);
    if(variant==1){attrs->SetMode(ON::locked_object);ON_3dmObjectAttributes hidden;hidden.SetMode(ON::hidden_object);attrs->ApplyParentalControl(hidden,layer,0x01U);}
    if(variant==0){ON_Xform translate=ON_Xform::TranslationTransformation(ON_3dVector(-10,-20,-30));
      auto second=add(model,reference(101,translate),32,false,L"second");
      const_cast<ON_3dmObjectAttributes*>(ON_ModelGeometryComponent::FromModelComponentRef(second,nullptr)->Attributes(nullptr))->SetColorSource(ON::color_from_layer);}
    if(variant==3){definition(model,103,L"Cycle",{41});add(model,reference(103,ON_Xform::IdentityTransformation),41,true,L"self");add(model,reference(103,ON_Xform::IdentityTransformation),42,false,L"cycle");}
    add(model,new ON_Point(ON_3dPoint(99,98,97)),50,false,L"ordinary");
    const char* names[]={"nested_blocks.3dm","hidden_blocks.3dm","missing_block.3dm","cyclic_block.3dm"};
    if(variant<2){
      for(auto type:{ON_ModelComponent::Type::ModelGeometry,ON_ModelComponent::Type::InstanceDefinition}){
        ONX_ModelComponentIterator it(model,type);
        for(const auto* component=it.FirstComponent();component!=nullptr;component=it.NextComponent()){
          if(!component->IsValid(nullptr))return 4;
          if(const auto* g=ON_ModelGeometryComponent::Cast(component))if(!g->Geometry(nullptr)->IsValid(nullptr))return 5;
        }
      }
    }
    if(!model.Write((directory+"/"+names[variant]).c_str(),80,nullptr))return 3;
  }
  ON::End();return 0;
}

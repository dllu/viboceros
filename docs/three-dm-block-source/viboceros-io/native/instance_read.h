// Independent block traversal; geometry decoding and affine application stay separate.
// Included inside the bridge's private namespace after read_geometry_object.
bool read_instance_geometry(const ONX_Model& source,const ON_ModelGeometryComponent& component,
    const ON_3dmObjectAttributes* parent,const std::vector<std::array<double,16>>& placements,
    std::vector<ON_UUID>& active,std::vector<BridgeObject>& output,std::string& error) {
  const auto* geometry=component.Geometry(nullptr);
  const auto* original=component.Attributes(nullptr);
  if(geometry==nullptr){error="block geometry is missing";return false;}
  ON_3dmObjectAttributes attributes=original==nullptr?ON_3dmObjectAttributes():*original;
  const bool original_visible=attributes.IsVisible();
  if(attributes.IsInstanceDefinitionObject()){
    attributes.SetMode(ON::normal_object);
  }
  if(parent!=nullptr){
    const ON_Layer* layer=ON_Layer::FromModelComponentRef(source.LayerFromIndex(parent->m_layer_index),nullptr);
    if(parent->Mode()==ON::locked_object)attributes.SetMode(ON::locked_object);
    attributes.ApplyParentalControl(*parent,layer==nullptr?ON_Layer::Default:*layer,0x03U);
    for(int i=0;i<parent->GroupCount();++i)attributes.AddToGroup(parent->GroupList()[i]);
    if(attributes.Name().IsEmpty())attributes.SetName(parent->Name(),true);
  }
  if(!original_visible){
    ON_3dmObjectAttributes hidden;hidden.SetMode(ON::hidden_object);
    attributes.ApplyParentalControl(hidden,ON_Layer::Default,0x01U);
  }
  if(const auto* instance=ON_InstanceRef::Cast(geometry)){
    const auto* instance_layer=ON_Layer::FromModelComponentRef(source.LayerFromIndex(attributes.m_layer_index),nullptr);
    const bool visible=attributes.IsVisible()&&(instance_layer==nullptr||instance_layer->IsVisible());
    if(instance_layer!=nullptr&&instance_layer->IsLocked())attributes.SetMode(ON::locked_object);
    if(!visible){ON_3dmObjectAttributes hidden;hidden.SetMode(ON::hidden_object);attributes.ApplyParentalControl(hidden,ON_Layer::Default,0x01U);}
    if(active.size()>=64){error="3DM block nesting exceeds 64 definitions";return false;}
    for(const auto& id:active)if(0==ON_UuidCompare(id,instance->m_instance_definition_uuid)){
      error="3DM block definitions contain a cycle";return false;
    }
    const auto ref=source.ComponentFromId(ON_ModelComponent::Type::InstanceDefinition,instance->m_instance_definition_uuid);
    const auto* definition=ON_InstanceDefinition::FromModelComponentRef(ref,nullptr);
    if(definition==nullptr){error="3DM block reference has no definition";return false;}
    if(!instance->m_xform.IsValid()||!instance->m_xform.IsAffine()){
      error="3DM block placement is not a finite affine transform";return false;
    }
    auto next=placements;std::array<double,16> matrix{};
    for(size_t i=0;i<16;++i)matrix[i]=instance->m_xform.m_xform[i/4][i%4];
    next.push_back(matrix);active.push_back(instance->m_instance_definition_uuid);
    const auto& ids=definition->InstanceGeometryIdList();
    if(ids.Count()==0&&definition->InstanceDefinitionType()!=ON_InstanceDefinition::IDEF_UPDATE_TYPE::Static){active.pop_back();return false;}
    for(unsigned i=0;i<ids.UnsignedCount();++i){
      const auto& child=source.ModelGeometryComponentFromId(ids[i]);
      if(!read_instance_geometry(source,child,&attributes,next,active,output,error)){active.pop_back();return false;}
    }
    active.pop_back();return true;
  }
  if(output.size()>=100000){error="3DM block expansion exceeds 100000 objects";return false;}
  BridgeObject result;
  if(!read_geometry_object(geometry,&attributes,result))return false;
  result.placements=placements;output.push_back(std::move(result));return true;
}

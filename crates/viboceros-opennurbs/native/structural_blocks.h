// Native definition/reference records; no geometry expansion on this path.
struct DefinitionIdLess { bool operator()(const ON_UUID& a,const ON_UUID& b)const{return ON_UuidCompare(a,b)<0;} };
using DefinitionIndex=std::map<ON_UUID,uint32_t,DefinitionIdLess>;

bool read_structural_object(const DefinitionIndex& definitions,const ON_Geometry* geometry,
    const ON_3dmObjectAttributes* original,BridgeObject& output,char* error,size_t capacity){
  ON_3dmObjectAttributes attributes=original==nullptr?ON_3dmObjectAttributes():*original;
  const bool visible=attributes.IsVisible();
  if(attributes.IsInstanceDefinitionObject()){
    attributes.SetMode(ON::normal_object);
    if(!visible){ON_3dmObjectAttributes hidden;hidden.SetMode(ON::hidden_object);attributes.ApplyParentalControl(hidden,ON_Layer::Default,0x01U);}
  }
  if(const auto* instance=ON_InstanceRef::Cast(geometry)){
    const auto found=definitions.find(instance->m_instance_definition_uuid);
    if(found==definitions.end()){set_error(error,capacity,"3DM block reference has no definition");return false;}
    if(!instance->m_xform.IsValid()||!instance->m_xform.IsAffine()){
      set_error(error,capacity,"3DM block placement is not finite affine data");return false;
    }
    if(!read_geometry_metadata(geometry,&attributes,output))return false;
    output.object_type=VIBO_OBJECT_INSTANCE;output.indices.push_back(found->second);
    for(int r=0;r<4;++r)for(int c=0;c<4;++c)output.coordinates.push_back(instance->m_xform[r][c]);
    return true;
  }
  return read_geometry_object(geometry,&attributes,output);
}

bool read_structural_blocks(const ONX_Model& source,std::vector<BridgeDefinition>& definitions,
    DefinitionIndex& indices,std::vector<BridgeObject>& objects,char* error,size_t capacity){
  std::vector<const ON_InstanceDefinition*> native;
  ONX_ModelComponentIterator iterator(source,ON_ModelComponent::Type::InstanceDefinition);
  for(const auto* component=iterator.FirstComponent();component!=nullptr;component=iterator.NextComponent()){
    const auto* definition=ON_InstanceDefinition::Cast(component);
    if(definition==nullptr){set_error(error,capacity,"invalid instance definition component");return false;}
    if(native.size()>=10000){set_error(error,capacity,"3DM block definition limit exceeded");return false;}
    if(definition->InstanceDefinitionType()!=ON_InstanceDefinition::IDEF_UPDATE_TYPE::Static){
      set_error(error,capacity,"linked block definitions are unsupported by structural import");return false;
    }
    if(!indices.emplace(definition->Id(),static_cast<uint32_t>(native.size())).second){
      set_error(error,capacity,"duplicate block definition ID");return false;
    }
    native.push_back(definition);
  }
  for(const auto* definition:native){
    BridgeDefinition record;record.name=utf8(definition->Name());record.first_object=objects.size();
    const auto& ids=definition->InstanceGeometryIdList();
    if(ids.UnsignedCount()>100000-objects.size()){set_error(error,capacity,"3DM definition member limit exceeded");return false;}
    for(unsigned i=0;i<ids.UnsignedCount();++i){
      const auto& component=source.ModelGeometryComponentFromId(ids[i]);
      const auto* geometry=component.Geometry(nullptr);
      if(geometry==nullptr){set_error(error,capacity,"block definition member is missing");return false;}
      BridgeObject object;
      if(!read_structural_object(indices,geometry,component.Attributes(nullptr),object,error,capacity)){
        set_error(error,capacity,"block definition member is missing or unsupported");return false;
      }
      objects.push_back(std::move(object));
    }
    record.object_count=objects.size()-record.first_object;definitions.push_back(std::move(record));
  }
  return true;
}

ON_InstanceRef* structural_instance_for(const ViboWriteObject& source,const std::vector<ON_UUID>& ids,
    const ViboWriteDefinition* definitions,std::string& error){
  if(source.index_count!=1||source.indices==nullptr||source.indices[0]>=ids.size()||
     source.coordinate_count!=16||source.coordinates==nullptr){error="invalid block reference payload";return nullptr;}
  ON_Xform transform;
  for(int r=0;r<4;++r)for(int c=0;c<4;++c)transform[r][c]=source.coordinates[4*r+c];
  if(!transform.IsValid()||!transform.IsAffine()){error="invalid affine block placement";return nullptr;}
  const auto& definition=definitions[source.indices[0]];
  ON_BoundingBox bounds(ON_3dPoint(definition.bounds[0],definition.bounds[1],definition.bounds[2]),
      ON_3dPoint(definition.bounds[3],definition.bounds[4],definition.bounds[5]));
  if(!bounds.IsValid()||!bounds.Transform(transform)){error="invalid placed block bounds";return nullptr;}
  auto* instance=new ON_InstanceRef();instance->m_instance_definition_uuid=ids[source.indices[0]];
  instance->m_xform=transform;instance->m_bbox=bounds;
  if(!instance->IsValid(nullptr)){delete instance;error="invalid native block instance";return nullptr;}
  return instance;
}

bool write_structural_definitions(ONX_Model& model,const ViboWriteDefinition* definitions,size_t count,
    const std::vector<ON_UUID>& ids,const std::vector<ON_UUID>& geometry,size_t top_start,char* error,size_t capacity){
  size_t expected=0;
  for(size_t i=0;i<count;++i){
    const auto& source=definitions[i];
    if(source.name==nullptr||source.first_object!=expected||source.object_count>top_start-expected){
      set_error(error,capacity,"invalid block definition object range");return false;
    }
    expected+=source.object_count;
    ON_InstanceDefinition definition;definition.SetId(ids[i]);definition.SetName(ON_wString(source.name));
    definition.SetInstanceDefinitionType(ON_InstanceDefinition::IDEF_UPDATE_TYPE::Static);
    definition.SetBoundingBox(ON_BoundingBox(ON_3dPoint(source.bounds[0],source.bounds[1],source.bounds[2]),
        ON_3dPoint(source.bounds[3],source.bounds[4],source.bounds[5])));
    ON_SimpleArray<ON_UUID> members;
    for(size_t j=source.first_object;j<expected;++j)members.Append(geometry[j]);
    definition.SetInstanceGeometryIdList(members);
    if(model.AddModelComponent(definition,false).IsEmpty()){
      set_error(error,capacity,"OpenNURBS could not add block definition");return false;
    }
  }
  if(expected!=top_start){set_error(error,capacity,"block definition object prefix differs from its ranges");return false;}
  return true;
}

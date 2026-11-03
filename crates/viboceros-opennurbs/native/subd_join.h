#pragma once
namespace vibo_subd {
// Rust chooses contacts, source order and orientation. Public native component
// tools retain the first archive and append the other control nets without a
// mesh conversion, preserving tags, sharpness and authored face channels.
inline std::unique_ptr<ON_SubD> join_assemble(const uint8_t* bytes,size_t byte_count,
    const size_t* sizes,size_t source_count,const uint32_t* maps,size_t map_count,
    const uint8_t* flips,bool crease) {
  require(bytes && sizes && maps && flips && source_count>=2 && source_count<=component_limit &&
    byte_count<=archive_limit && map_count<=component_limit,"Invalid SubD join assembly");
  std::unique_ptr<ON_SubD> output;
  std::vector<ON_SubDVertex*> vertices;
  std::map<std::pair<uint32_t,uint32_t>,ON_SubDEdge*> edges;
  size_t offset=0,corner_offset=0;
  for(size_t source_index=0;source_index<source_count;++source_index) {
    require(offset<=byte_count && sizes[source_index]<=byte_count-offset && flips[source_index]<=1,"Invalid SubD join archive span");
    auto source=decode(bytes+offset,sizes[source_index]);offset+=sizes[source_index];
    require(source->VertexCount()<=map_count-corner_offset,"Missing SubD join vertex mapping");
    std::vector<const ON_SubDVertex*> vv;auto vit=source->VertexIterator();
    for(auto* v=vit.FirstVertex();v;v=vit.NextVertex())vv.push_back(v);
    std::sort(vv.begin(),vv.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});
    if(source_index==0) {
      require(flips[0]==0,"Cannot reverse the SubD join seed");output=std::move(source);
      std::unordered_map<const ON_SubDVertex*,uint32_t> index;
      for(size_t i=0;i<vv.size();++i) {
        require(maps[i]==i,"Cannot remap the SubD join seed");
        vertices.push_back(const_cast<ON_SubDVertex*>(vv[i]));index.emplace(vv[i],static_cast<uint32_t>(i));
      }
      auto eit=output->EdgeIterator();
      for(auto* e=eit.FirstEdge();e; e=eit.NextEdge()) {
        auto a=index.at(e->m_vertex[0]),b=index.at(e->m_vertex[1]);
        require(edges.emplace(std::minmax(a,b),const_cast<ON_SubDEdge*>(e)).second,"Parallel SubD join seed edges are ambiguous");
      }
    } else {
      if(flips[source_index])require(source->ReverseOrientation(),"Cannot orient SubD join source");
      std::unordered_map<const ON_SubDVertex*,ON_SubDVertex*> vertex_map;
      std::unordered_map<const ON_SubDVertex*,uint32_t> ordinals;
      for(size_t i=0;i<vv.size();++i) {
        const uint32_t target=maps[corner_offset+i];
        require(target<=vertices.size(),"Invalid SubD join vertex order");
        if(target==vertices.size()) {
          const auto p=vv[i]->ControlNetPoint();
          auto* v=output->AddVertex(vv[i]->m_vertex_tag,&p.x);require(v,"Cannot append SubD join control");vertices.push_back(v);
        } else vertices[target]->m_vertex_tag=ON_SubDVertexTag::Unset;
        vertex_map.emplace(vv[i],vertices[target]);ordinals.emplace(vv[i],target);
      }
      std::unordered_map<const ON_SubDEdge*,ON_SubDEdge*> edge_map;
      std::vector<ON_SubDEdge*> joined;
      std::vector<const ON_SubDEdge*> ee;auto eit=source->EdgeIterator();
      for(auto* e=eit.FirstEdge();e;e=eit.NextEdge())ee.push_back(e);
      std::sort(ee.begin(),ee.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});
      std::set<std::pair<unsigned,unsigned>> source_edges;
      for(auto* e:ee) {
        require(source_edges.insert(std::minmax(e->m_vertex[0]->m_id,e->m_vertex[1]->m_id)).second,
          "Parallel SubD join source edges need explicit face-edge identities");
        const auto a=ordinals.at(e->m_vertex[0]),b=ordinals.at(e->m_vertex[1]);
        require(a!=b,"Collapsed joined SubD edge");
        const auto key=std::make_pair(std::min(a,b),std::max(a,b));
        auto found=edges.find(key);ON_SubDEdge* edge=nullptr;
        if(found!=edges.end()) {edge=found->second;joined.push_back(edge);}
        else {
          edge=output->AddEdge(e->m_edge_tag,vertex_map.at(e->m_vertex[0]),vertex_map.at(e->m_vertex[1]));
          require(edge,"Cannot append joined SubD edge");
          if(e->IsSmooth())edge->SetSharpnessForExperts(e->Sharpness(false));
          edges.emplace(key,edge);
        }
        edge_map.emplace(e,edge);
      }
      std::vector<const ON_SubDFace*> ff;auto fit=source->FaceIterator();
      for(auto* f=fit.FirstFace();f;f=fit.NextFace())ff.push_back(f);
      std::sort(ff.begin(),ff.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});
      for(auto* f:ff) {
        ON_SimpleArray<ON_SubDEdgePtr> boundary;
        for(unsigned j=0;j<f->EdgeCount();++j) {
          auto* edge=edge_map.at(f->Edge(j));auto* vertex=vertex_map.at(f->Vertex(j));
          boundary.Append(ON_SubDEdgePtr::Create(edge,edge->m_vertex[0]==vertex?0:1));
        }
        auto* face=output->AddFace(boundary);require(face,"Cannot append joined SubD face");
        face->SetPerFaceColor(f->PerFaceColor());face->SetMaterialChannelIndex(f->MaterialChannelIndex());
        if(f->TexturePointsAreSet()) {
          require(output->AllocateFaceTexturePoints(face)>=f->EdgeCount(),"Cannot retain joined SubD texture points");
          for(unsigned j=0;j<f->EdgeCount();++j)require(face->SetTexturePoint(j,f->TexturePoint(j)),"Cannot copy joined SubD texture point");
        }
      }
      for(auto* edge:joined) {
        edge->m_edge_tag=crease || edge->FaceCount()!=2?ON_SubDEdgeTag::Crease:ON_SubDEdgeTag::Smooth;
        edge->ClearSharpnessForExperts();
      }
      output->UpdateAllTagsAndSectorCoefficients(false);output->ClearEvaluationCache();
    }
    corner_offset+=vv.size();
    check(*output);
  }
  require(offset==byte_count && corner_offset==map_count,"Trailing SubD join data");
  auto fit=output->FaceIterator();
  for(auto* face=fit.FirstFace();face;face=fit.NextFace()) {
    auto* editable=const_cast<ON_SubDFace*>(face);editable->ClearPackId();editable->ClearPackRect();
  }
  return output;
}
} // namespace vibo_subd

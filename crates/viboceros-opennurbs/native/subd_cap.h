#ifndef VIBOCEROS_SUBD_CAP_H
#define VIBOCEROS_SUBD_CAP_H
extern "C" int vibo_subd_append_caps(const uint8_t* bytes,size_t count,
    const double* points,size_t point_count,const uint32_t* sizes,size_t face_count,
    const uint32_t* corners,size_t corner_count,const size_t* edge_indices,size_t edge_count,
    int crease,ViboSubDResult** out,char* error,size_t capacity) {
  return guard(out,error,capacity,[&] {
    require((crease==0 || crease==1) && edge_indices && edge_count>0 &&
      edge_count<=4*vibo_subd::component_limit,"Invalid SubD cap boundaries");
    auto subd=vibo_subd::decode(bytes,count);
    std::vector<const ON_SubDEdge*> edges;
    auto eit=subd->EdgeIterator();
    for(auto* e=eit.FirstEdge();e;e=eit.NextEdge()) edges.push_back(e);
    std::sort(edges.begin(),edges.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});
    ON_SimpleArray<ON_SubDComponentPtr> boundary;
    for(size_t i=0;i<edge_count;++i) {
      require(edge_indices[i]<edges.size() && edges[edge_indices[i]]->FaceCount()==1,
        "Cap selection is not an original naked edge");
      boundary.Append(ON_SubDComponentPtr::Create(edges[edge_indices[i]]));
    }
    vibo_subd::append_polygons(*subd,points,point_count,sizes,face_count,corners,corner_count);
    if(crease) subd->SetEdgeTags(boundary.Array(),boundary.UnsignedCount(),ON_SubDEdgeTag::Crease);
    subd->UpdateAllTagsAndSectorCoefficients(false);
    subd->ClearEvaluationCache();
    if(subd->SolidOrientation()==-1)
      require(subd->ReverseOrientation(),"Cannot reverse inward capped SubD");
    return extract(*subd);
  });
}
#endif

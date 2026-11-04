#ifndef VIBOCEROS_SUBD_SHARPNESS_H
#define VIBOCEROS_SUBD_SHARPNESS_H
namespace vibo_subd {
template<class F> int sharpness_guard(char* error,size_t capacity,F run) {
  if(error && capacity)error[0]=0;
  try {vibo_begin_open_nurbs();run();return 1;}
  catch(const std::exception& exception) {
    if(error && capacity){const auto n=std::min(capacity-1,std::strlen(exception.what()));
      std::memcpy(error,exception.what(),n);error[n]=0;}
  }catch(...) {
    if(error && capacity){const char* message="Unknown sharpness failure";
      const auto n=std::min(capacity-1,std::strlen(message));std::memcpy(error,message,n);error[n]=0;}
  }
  return 0;
}
}
extern "C" int vibo_subd_sharpness_from_percent(double percentage,double* out,char* error,size_t capacity) {
  return vibo_subd::sharpness_guard(error,capacity,[&] {
    vibo_subd::require(out && std::isfinite(percentage) && percentage>=0. && percentage<=100.,
      "Soft crease percentage outside 0 through 100");
    *out=ON_SubDEdgeSharpness::SharpnessFromSliderValue(ON_Interval(0.,100.),percentage,ON_DBL_QNAN);
    vibo_subd::require(ON_SubDEdgeSharpness::IsValidValue(*out,false),"Invalid soft crease percentage conversion");
  });
}
extern "C" int vibo_subd_chain_sharpness(const double* ends,size_t count,double* out,char* error,size_t capacity) {
  return vibo_subd::sharpness_guard(error,capacity,[&] {
    vibo_subd::require(ends && out && count>0 && count<=4*vibo_subd::component_limit &&
      std::isfinite(ends[0]) && std::isfinite(ends[1]) && ends[0]>=0. && ends[0]<=4. && ends[1]>=0. && ends[1]<=4.,
      "Invalid SubD sharpness interval or edge count");
    ON_SimpleArray<ON_SubDEdgeSharpness> values;
    vibo_subd::require(ON_SubDEdgeSharpness::SetEdgeChainSharpness(ON_Interval(ends[0],ends[1]),
      static_cast<unsigned>(count),values)>0 && values.UnsignedCount()==count,"Cannot interpolate SubD chain sharpness");
    for(size_t i=0;i<count;++i)for(unsigned k=0;k<2;++k)out[2*i+k]=values[static_cast<int>(i)].EndSharpness(k);
  });
}
extern "C" int vibo_subd_sharpness_refinement(const double* values,size_t count,uint32_t* out,char* error,size_t capacity) {
  return vibo_subd::sharpness_guard(error,capacity,[&] {
    vibo_subd::require(count<=4*vibo_subd::component_limit && (count==0 || (values && out)),
      "Invalid SubD sharpness subdivision input");
    for(size_t i=0;i<count;++i) {
      for(unsigned end=0;end<2;++end)
        vibo_subd::require(std::isfinite(values[2*i+end]) && values[2*i+end]>=0. && values[2*i+end]<=4.,
          "Invalid SubD sharpness interval");
      const auto original=ON_SubDEdgeSharpness::FromInterval(values[2*i],values[2*i+1]);
      for(unsigned end=0;end<2;++end) {
        auto value=original;uint32_t level=0;
        while(!value.IsZero() && level<4) {value=value.Subdivided(end);++level;}
        vibo_subd::require(value.IsZero(),"Unresolved SubD sharpness interval");
        out[3*i+end]=level;
      }
      uint32_t breaks=0;
      if(!original.IsZero()) {
        std::array<ON_SubDEdgeSharpness,8> current{},next{};current[0]=original;
        for(unsigned level=0;level<4;++level) {
          const unsigned size=1u<<level;
          if(level>=2)for(unsigned j=0;j<size;++j)if(!current[j].IsZero()) {
            const int width=32/static_cast<int>(size),step=width/4;
            const int a=std::max(0,(static_cast<int>(j)-1)*width);
            const int b=std::min(32,(static_cast<int>(j)+2)*width);
            for(int k=a;k<=b;k+=step)if(k>0 && k<32)breaks|=uint32_t(1)<<k;
          }
          if(level==3)break;
          for(unsigned j=0;j<size;++j) {next[2*j]=current[j].Subdivided(0);next[2*j+1]=current[j].Subdivided(1);}
          current=next;
        }
      }
      out[3*i+2]=breaks;
    }
  });
}
#endif

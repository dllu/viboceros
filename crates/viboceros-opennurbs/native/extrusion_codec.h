#ifndef VIBOCEROS_EXTRUSION_CODEC_H
#define VIBOCEROS_EXTRUSION_CODEC_H
// Public OpenNURBS extrusion archives preserve profiles, caps and miter planes.
#include "opennurbs_public.h"
#include <cstring>
#include <memory>
#include <stdexcept>
#include <vector>
namespace vibo_extrusion {
constexpr size_t archive_limit=64U*1024U*1024U;
inline void require(bool ok,const char* message) { if(!ok) throw std::runtime_error(message); }
inline void check(const ON_Extrusion& extrusion) {
  require(extrusion.IsValid() && extrusion.ProfileCount()>0 && extrusion.ProfileCount()<=100000,"Invalid extrusion or profile budget exceeded");
}
inline void u32(std::vector<uint8_t>& bytes,uint32_t n) {
  for(unsigned i=0;i<4;++i) bytes.push_back(static_cast<uint8_t>(n>>(8*i)));
}
inline uint32_t u32(const uint8_t* p) {
  uint32_t n=0;for(unsigned i=0;i<4;++i)n|=uint32_t(p[i])<<(8*i);return n;
}
inline std::vector<uint8_t> encode(const ON_Extrusion& extrusion) {
  check(extrusion);const unsigned library=ON::Version();
  ON_Write3dmBufferArchive archive(0,archive_limit-16,80,library);
  // File geometry user text is owned by the adapter, independently of the
  // immutable shape representation. Do not resurrect removed keys on save.
  ON_Extrusion geometry(extrusion);ON_ClassArray<ON_wString> keys;geometry.GetUserStringKeys(keys);
  for(int i=0;i<keys.Count();++i) require(geometry.SetUserString(keys[i],nullptr),"Cannot detach managed extrusion user text");
  require(geometry.Write(archive),"Cannot archive extrusion");
  std::vector<uint8_t> bytes{'V','I','B','O','E','X','T','R'};
  u32(bytes,80);u32(bytes,library);
  const auto* p=static_cast<const uint8_t*>(archive.Buffer());
  bytes.insert(bytes.end(),p,p+archive.SizeOfArchive());return bytes;
}
inline std::unique_ptr<ON_Extrusion> decode(const uint8_t* bytes,size_t count) {
  require(bytes && count>16 && count<=archive_limit && std::memcmp(bytes,"VIBOEXTR",8)==0,"Invalid extrusion archive envelope");
  require(u32(bytes+8)==80,"Unsupported extrusion archive version");
  ON_Read3dmBufferArchive archive(count-16,bytes+16,false,80,u32(bytes+12));
  auto extrusion=std::make_unique<ON_Extrusion>();
  require(extrusion->Read(archive) && archive.CurrentPosition()==count-16,"Invalid extrusion archive");
  check(*extrusion);return extrusion;
}
}
#endif

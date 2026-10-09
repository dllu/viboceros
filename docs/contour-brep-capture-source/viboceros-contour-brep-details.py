from pathlib import Path
import json,copy,shutil
from unittest.mock import patch
from tools.rhino_oracle.client import OracleClient
root=Path('/home/dllu/proj/viboceros');f=json.loads((root/'tools/rhino_oracle/fixtures/contour_brep.json').read_text());ops=[]
for name in ['full-face-x','trimmed-rectangle','trimmed-hole','box-x','box-diagonal']:
 ops.append(copy.deepcopy(next(o for o in f['operations']if o['id']==name)))
for name,z in [('reversed-face',0),('trimmed-rectangle-offset',.5)]:
 op=copy.deepcopy(next(o for o in f['operations']if o['id']==('full-face-x'if name=='reversed-face'else'trimmed-rectangle')));op['id']=name
 if name=='reversed-face':op['sources'][0]['reversed']=True
 else:op['start'][2]=z;op['end'][2]=z
 ops.append(op)
base=copy.deepcopy(f['operations'][0]);base['group']=True
for name,source in [('single-grouped-line',dict(type='line',start=[-2,0,0],end=[2,0,0])),('single-grouped-surface',base['sources'][0]['surface']),('single-grouped-box',dict(type='box_brep',min=[-2,-2,-2],max=[2,2,2]))]:
 op=copy.deepcopy(base);op['id']=name;op['sources']=[source]
 if name=='single-grouped-surface':op['sources'][0]['type']='surface'
 ops.append(op)
request=dict(protocol_version=1,iterations=1,operations=ops)
Path('/tmp/viboceros-contour-brep-details-request.json').write_text(json.dumps(request,indent=2)+'\n')
source=(root/'tools/rhino_oracle/contour_probe.py').read_text();source=source.replace('        color=attributes.ObjectColor',"        native_type=str(geometry.GetType().Name)\n        native_controls=None\n        if isinstance(geometry,Rhino.Geometry.Curve):\n            ok,polyline=geometry.TryGetPolyline()\n            if ok:native_controls=[xyz(point)for point in polyline]\n        color=attributes.ObjectColor").replace('return dict(kind=kind,','return dict(native_type=native_type,native_controls=native_controls,kind=kind,')
source=source.replace('ids=[];layers=[];outputs=[]','ids=[];layers=[];outputs=[];native_faces=[]')
source=source.replace("            identifier=document.Objects.Add(geometry,attributes);", "            if isinstance(geometry,Rhino.Geometry.Brep):\n                native_faces.append([dict(index=int(face.FaceIndex),center=xyz(face.PointAt(face.Domain(0).Mid,face.Domain(1).Mid)),normal=xyz(face.NormalAt(face.Domain(0).Mid,face.Domain(1).Mid)),domains=[[float(face.Domain(i).T0),float(face.Domain(i).T1)]for i in range(2)],reversed=bool(face.OrientationIsReversed)) for face in geometry.Faces])\n            identifier=document.Objects.Add(geometry,attributes);")
source=source.replace('return dict(succeeded=bool(succeeded),outputs=records,','return dict(native_faces=native_faces,succeeded=bool(succeeded),outputs=records,')
Path('/tmp/viboceros-contour-brep-details-helper.py').write_text(source)
original_copy=shutil.copyfile
def copyfile(src,dst,*a,**kw):
 if Path(src).name=='contour_probe.py':Path(dst).write_text(source);return dst
 return original_copy(src,dst,*a,**kw)
with patch('tools.rhino_oracle.client.shutil.copyfile',copyfile):response=OracleClient(settings_scheme='VibocerosOracleContourBrepDetails20261009').run_rhino(request,240)
Path('/tmp/viboceros-contour-brep-details-native.json').write_text(json.dumps(response,indent=2)+'\n')

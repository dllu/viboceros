from pathlib import Path
import json,copy,shutil
from unittest.mock import patch
from tools.rhino_oracle.client import OracleClient
root=Path('/home/dllu/proj/viboceros')
fixture=json.loads((root/'tools/rhino_oracle/fixtures/contour_command.json').read_text())
ops=[copy.deepcopy(o)for o in fixture['operations']if o['id']in ('surface-x','surface-y','saddle-cuts','translated-plane')]
base=next(o for o in ops if o['id']=='surface-x')
for name,start,end in [('plane-base-z1',[0,0,1],[1,0,1]),('plane-base-y6',[0,6,0],[1,6,0]),('plane-base-z-small',[0,0,0.0001],[1,0,0.0001]),('plane-base-z001',[0,0,.01],[1,0,.01]),('plane-base-z01',[0,0,.1],[1,0,.1]),('plane-base-z05',[0,0,.5],[1,0,.5]),('plane-base-z-negative',[0,0,-1],[1,0,-1])]:ops.append(dict(copy.deepcopy(base),id=name,start=start,end=end))
ops=[dict(copy.deepcopy(op),id='repeat-'+str(i)+'-'+op['id']) for i in range(3) for op in ops if op['id'] in ('plane-base-z-small','plane-base-z001','plane-base-z01','plane-base-z05')]
request=dict(protocol_version=1,iterations=1,operations=ops)
Path('/tmp/viboceros-contour-domain-repeat-request.json').write_text(json.dumps(request,indent=2)+'\n')
source=(root/'tools/rhino_oracle/contour_probe.py').read_text()
source=source.replace('        color=attributes.ObjectColor', '''        native_type=str(geometry.GetType().Name)
        native_linear=bool(geometry.IsLinear(float(tolerance.get("absolute",1e-9)))) if isinstance(geometry,Rhino.Geometry.Curve) else False
        native_degree=int(geometry.Degree) if isinstance(geometry,Rhino.Geometry.Curve) else None
        native_controls=[xyz(point.Location) for point in geometry.Points] if isinstance(geometry,Rhino.Geometry.NurbsCurve) else None
        native_knots=[float(knot) for knot in geometry.Knots] if isinstance(geometry,Rhino.Geometry.NurbsCurve) else None
        color=attributes.ObjectColor''')
source=source.replace('return dict(kind=kind,points=points,domain=domain,','return dict(native_controls=native_controls,native_knots=native_knots,native_type=native_type,native_linear=native_linear,native_degree=native_degree,kind=kind,points=points,domain=domain,')
Path('/tmp/viboceros-contour-domain-repeat-helper.py').write_text(source)
original_copy=shutil.copyfile
def copyfile(src,dst,*a,**kw):
    if Path(src).name=='contour_probe.py':Path(dst).write_text(source);return dst
    return original_copy(src,dst,*a,**kw)
with patch('tools.rhino_oracle.client.shutil.copyfile',copyfile):
    response=OracleClient(settings_scheme='VibocerosOracleContourDomainRepeats20261009').run_rhino(request,240)
Path('/tmp/viboceros-contour-domain-repeat-native.json').write_text(json.dumps(response,indent=2)+'\n')
print('captured',len(response['results']),'recipes')

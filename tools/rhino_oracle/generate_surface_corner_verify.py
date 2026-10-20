"""Query captured SDK/command corner curves at an independent quadratic point."""
import gzip,hashlib,json
from pathlib import Path
HERE=Path(__file__).resolve().parent
CAPTURES=['surface_curve_offset_corners.json','surface_offset_corners.json']
def request():
    operations=[]
    for name in CAPTURES:
        rows=json.loads((HERE/'observations'/name).read_text())['results']
        value=next(x['value']for x in rows if x['id']=='corner_polycurve')
        curve=value['outputs'][0]['definition']if name.startswith('surface_curve')else value['after_script'][2]['curve']
        source=value['curve']if name.startswith('surface_curve')else value['before'][0]['curve']
        assert source['degree']==3 and [p['point']for p in source['control_points']][-4:]==[[0.,-2.,0.],[0.,-1.,0.],[2.,2.,0.],[3.,3.,0.]]
        assert all(p['weight']==1. for p in source['control_points'])
        # A=(0,-2.5), apex=(0.5,-2.5), B=(0.5,-2), rho=1/2.
        # At s=5/8 this polynomial Bernstein point is exactly binary64.
        s=5./8.;point=[s-.5*s*s,-2.5+.5*s*s,0.]
        operations.append(dict(op='nurbs_curve_closest_point',id='sdk_bridge'if name.startswith('surface_curve')else'command_bridge',degree=curve['degree'],control_points=curve['control_points'],knots=curve['knots'],target=point))
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-9,relative=1e-12,angular=1e-9),operations=operations)
def main():
    from .client import OracleClient
    q=request();path=HERE/'fixtures/surface_offset_corner_verify.json';path.write_text(json.dumps(q,indent=2)+'\n')
    names=['generate_surface_corner_verify.py','client.py','rhino_worker.py','structural_sdk.py']
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={n:digest(HERE/n)for n in names};captures={n:digest(HERE/'observations'/n)for n in CAPTURES}
    print('Verifying captured corner locations',flush=True)
    r=OracleClient(settings_scheme='VibocerosOracleCornerVerify20261020').run_rhino(q,timeout=180)
    assert sources=={n:digest(HERE/n)for n in names}
    assert captures=={n:digest(HERE/'observations'/n)for n in CAPTURES}
    r['provenance']=dict(request_sha256=digest(path),captures=captures,sources=sources)
    (HERE/'observations/surface_offset_corner_verify.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    for name,sha in sources.items():
        p=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not p.exists():p.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Recorded native corner verification',flush=True)
if __name__=='__main__':main()

"""Independently query captured native curves at analytic normal-family points."""
import gzip,hashlib,json,math
from pathlib import Path
HERE=Path(__file__).resolve().parent
def request():
    rows={r['id']:r['value']for r in json.loads((HERE/'observations/surface_offset_through_point.json').read_text())['results']}
    operations=[]
    def add(name,curve,point):
        operations.append(dict(op='nurbs_curve_closest_point',id=name,degree=curve['degree'],control_points=curve['control_points'],knots=curve['knots'],target=point))
    curve=rows['through_bezier']['after_script'][2]['curve']
    source=rows['through_bezier']['before'][0]['curve']
    assert source['degree']==3 and len(source['knots'])==8
    assert [p['point']for p in source['control_points']]==[[-4.,0.,0.],[-2.,3.,0.],[2.,-3.,0.],[4.,0.,0.]]
    assert all(p['weight']==1. for p in source['control_points'])
    assert curve['control_points'][0]['point'][0]==-10.
    distance=(curve['control_points'][0]['point'][1]+9.)*2./math.sqrt(13.)
    assert abs(distance-1.7982112232921417)<1e-10
    for i,t in enumerate([.8,.84,.876,.92,.97]):
        x=-4.+6.*t+6.*t*t-4.*t*t*t;y=9.*t-27.*t*t+18.*t*t*t
        dx=6.+12.*t-12.*t*t;dy=9.-54.*t+54.*t*t;speed=math.hypot(dx,dy)
        add('bezier_normal_'+str(i),curve,[x-distance*dy/speed,y+distance*dx/speed,0.])
    add('bezier_picked_point',curve,[0.,2.,0.])
    for i,point in enumerate([[3.,0.,3.],[0.,3.,3.]]):
        add('cylinder_pick_'+str(i),rows['through_cylinder']['after_script'][i+2]['curve'],point)
    add('sphere_pick',rows['through_sphere']['after_script'][2]['curve'],[4.,0.,0.])
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-8,relative=1e-12,angular=1e-8),operations=operations)
def main():
    from .client import OracleClient
    q=request();path=HERE/'fixtures/surface_offset_through_verify.json';path.write_text(json.dumps(q,indent=2)+'\n')
    names=['generate_surface_through_verify.py','client.py','rhino_worker.py','structural_sdk.py']
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={n:digest(HERE/n)for n in names}
    print('Verifying',len(q['operations']),'native curve locations',flush=True)
    r=OracleClient(settings_scheme='VibocerosOracleThroughVerify20261020').run_rhino(q,timeout=180)
    assert sources=={n:digest(HERE/n)for n in names}
    r['provenance']=dict(request_sha256=digest(path),capture_sha256=digest(HERE/'observations/surface_offset_through_point.json'),sources=sources)
    (HERE/'observations/surface_offset_through_verify.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    for name,sha in sources.items():
        p=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not p.exists():p.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Recorded native curve verification',flush=True)
if __name__=='__main__':main()

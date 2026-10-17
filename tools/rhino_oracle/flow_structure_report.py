"""Analyze Flow lookup and fixed-direction structure; requires NumPy for SVD.

Native batch parameters and fitted directions are diagnostic inputs only.
They never initialize production morphs or change the Rust geometry kernel.
"""
import argparse
import json
import math
from pathlib import Path
import numpy as np

BASE=Path(__file__).parent


def array(value):
    a=np.asarray(value,dtype=float)
    if not np.isfinite(a).all():raise ValueError('nonfinite structural observation')
    return a


def bezier(recipe,parameters):
    degree=recipe['degree'];controls=recipe['control_points']
    if recipe['type']!='nurbs'or len(controls)!=degree+1 or recipe['knots']!=[0]*(degree+1)+[1]*(degree+1)or any(c['weight']!=1 for c in controls):raise ValueError('structural lookup witness must be a nonrational Bezier')
    t=array(parameters)[:,None];c=array([p['point']for p in controls])
    return sum(math.comb(degree,i)*(1-t)**(degree-i)*t**i*c[i]for i in range(degree+1))


def fixed_direction(points):
    points=array(points)
    if points.ndim!=2 or points.shape[1]!=3 or len(points)%3:raise ValueError('incomplete native basis')
    y=points[1::3]-points[::3];z=points[2::3]-points[::3];t=np.cross(y,z)
    if max(np.max(abs(np.linalg.norm(y,axis=1)-1)),np.max(abs(np.linalg.norm(z,axis=1)-1)),np.max(abs(np.sum(y*z,axis=1))))>1e-10:raise ValueError('native witness is not an orthonormal basis')
    # A meridian frame rotated through a fixed angle has an axis perpendicular
    # to b + k cross tangent. Fit that homogeneous relation on a subset, then
    # check the shared-direction model against every observation.
    count=min(17 if len(y)>64 else 9,len(y))
    indices=np.unique(np.linspace(0,len(y)-1,count,dtype=int))
    design=np.c_[np.cross(t[indices],y[indices]),y[indices]]
    _,singular,v=np.linalg.svd(design,full_matrices=False)
    if len(singular)!=6 or singular[-2]<1e-10:raise ValueError('insufficient directions for a unique fitted pole')
    k,b=v[-1,:3],v[-1,3:];length=np.linalg.norm(b)
    if length<1e-12:raise ValueError('degenerate fixed-direction fit')
    k,b=k/length,b/length
    if b[1]<0:k,b=-k,-b
    sy=np.cross(b,[1.,0,0]);sy/=np.linalg.norm(sy);sz=np.cross([1.,0,0],sy)
    ty=np.cross(b,t);ty/=np.linalg.norm(ty,axis=1)[:,None];tz=np.cross(t,ty)
    yy=sy[1]*ty+sz[1]*tz;zz=sy[2]*ty+sz[2]*tz
    return dict(fitted_direction=b.tolist(),training_sections=len(indices),checked_sections=len(y),
                cross_coefficient_parallel_error=float(np.linalg.norm(np.cross(k,b))),
                max_shared_direction_basis_error=float(max(np.max(abs(yy-y)),np.max(abs(zz-z)))))


def report(base=BASE):
    def read(path):return json.loads((base/path).read_text())
    data={}
    for name in ('flow_density','flow_coupled_frames','flow_up_lookup'):
        q=read('fixtures/'+name+'.json');n=read('observations/'+name+'_rhino_20261016.json')
        if q['protocol_version']!=n['protocol_version']or n['engine']!='rhino'or [o['id']for o in q['operations']]!=[r['id']for r in n['results']]:raise ValueError('structural identities/protocol differ')
        if len({o['id']for o in q['operations']})!=len(q['operations']):raise ValueError('duplicate structural identity')
        for op,row in zip(q['operations'],n['results']):
            if len(op['points'])!=len(row['value']['sdk_points']):raise ValueError('structural point count differs')
            array(row['value']['sdk_points'])
        data[name]=(q,n)
    q,n=data['flow_up_lookup'];bulk=n['results'][0]['value']['diagnostics']['target']['length_batches']
    lookup=next(r for r in bulk if r['divisions']==840 and r['fractional_tolerance']==1e-4)
    parameters=array(lookup['parameters'])
    if len(parameters)!=841 or np.any(np.diff(parameters)<=0)or parameters[0]!=0 or parameters[-1]!=1:raise ValueError('invalid public inverse table')
    inverse_grid=np.linspace(0,1,len(parameters));lookups=[]
    for request,response in [data['flow_density'],(dict(operations=[q['operations'][3]]),dict(results=[n['results'][3]]))]:
        for op,row in zip(request['operations'],response['results']):
            stride=3 if op['id']=='dense-basis'else 1
            points=array(op['points'])[::stride]
            if np.max(abs(points[:,1:]))!=0:raise ValueError('lookup witnesses must lie on the source line')
            fractions=points[:,0]/10
            predicted=bezier(op['target'],np.interp(fractions,inverse_grid,parameters))
            actual=array(row['value']['sdk_points'])[::stride]
            lookups.append(dict(id=op['id'],samples=len(points),max_coordinate_error=float(np.max(abs(predicted-actual)))))
    dense=data['flow_density'][1]['results'][1]['value']['sdk_points']
    directions=[dict(id='dense-basis',**fixed_direction(dense))]
    directions.extend(dict(id=row['id'],**fixed_direction(row['value']['sdk_points']))for row in n['results'][1:3])
    old={r['id']:r['value']['sdk_points']for r in read('observations/flow_frames_rhino_20261016.json')['results']}
    representation=[];cq,cn=data['flow_coupled_frames']
    for op,row in zip(cq['operations'][1:],cn['results'][1:]):
        baseline='space-dense'if op['id'].startswith('base-')else 'source-space-start'
        a,b=array(row['value']['sdk_points']),array(old[baseline])
        if a.shape!=b.shape:raise ValueError('guide representation point shapes differ')
        representation.append(dict(id=op['id'],max_coordinate_error=float(np.max(abs(a-b)))))
    batch_errors=[];centers=array(cn['results'][0]['value']['sdk_points'])[::3]
    for row in cn['results'][0]['value']['diagnostics']['target']['length_batches']:
        batch_errors.append(dict(divisions=100,fractional_tolerance=row['fractional_tolerance'],max_coordinate_error=float(np.max(abs(array(row['points'])-centers)))))
    return dict(cases=sum(len(q['operations'])for q,_ in data.values()),production_kernel_changed=False,
                spatial_flow_qualified=False,native_values_used_as_kernel_inputs=False,
                lookup=dict(observed_intervals=840,public_batch_fractional_tolerance=1e-4,comparisons=lookups),
                fixed_direction_candidates=directions,guide_representation_comparisons=representation,
                batch_100_comparisons=batch_errors)


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('output',type=Path);args=p.parse_args()
    args.output.write_text(json.dumps(report(),indent=2,allow_nan=False)+'\n')


if __name__=='__main__':main()

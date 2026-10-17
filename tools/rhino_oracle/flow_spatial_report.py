"""Analyze public Flow length/frame diagnostics without changing kernel inputs.

The dense witness is a line-to-cubic-Bezier map with an authored orthogonal
unit basis at every normalized length. Other recipes retain classification
and frame queries for refinement, rotation and scale comparisons.
"""
import argparse
import json
import math
from pathlib import Path


def sub(a, b):return [x-y for x,y in zip(a,b)]
def dot(a, b):return sum(x*y for x,y in zip(a,b))
def cross(a, b):return [a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]]
def unit(v):
    n=math.hypot(*v)
    if not n>0 or not math.isfinite(n):raise ValueError('invalid diagnostic axis')
    return [x/n for x in v]
def maximum_delta(a, b):
    if len(a)!=len(b):raise ValueError('diagnostic shapes differ')
    return max(abs(x-y)for p,q in zip(a,b)for x,y in zip(p,q))
def wrap(a):return math.atan2(math.sin(a),math.cos(a))
def finite_point(p):return isinstance(p,list)and len(p)==3 and all(type(x)in(int,float)and math.isfinite(x)for x in p)


def report(request, native):
    if request['protocol_version']!=native['protocol_version']or native['engine']!='rhino':raise ValueError('diagnostic engine/protocol differs')
    operations=request['operations'];results=native['results']
    if [o['id']for o in operations]!=[r['id']for r in results]or len({o['id']for o in operations})!=len(operations):raise ValueError('diagnostic identities differ')
    summaries=[]
    for op,result in zip(operations,results):
        value=result['value'];count=op['diagnostic_length_samples']
        if value['command']is not None or len(value['sdk_points'])!=len(op['points'])or not all(finite_point(p)for p in value['sdk_points']):raise ValueError('unexpected diagnostic map')
        for role in ('base','target'):
            diag=value['diagnostics'][role]
            if len(diag['length_rows'])!=count or len(diag['length_batch'])!=count:raise ValueError('missing diagnostic rows')
            previous=None
            for i,(row,batch)in enumerate(zip(diag['length_rows'],diag['length_batch'])):
                if row['fraction']!=i/(count-1)or not math.isfinite(row['t'])or(previous is not None and row['t']<=previous):raise ValueError('invalid length parameters')
                if not finite_point(row['point'])or not finite_point(row['tangent']):raise ValueError('nonfinite length query')
                previous=row['t']
                for frame in (row['frame'],row['perp'],batch):
                    for key in ('origin','x','y','z'):
                        if not finite_point(frame[key]):raise ValueError('nonfinite public frame')
                    for key in ('x','y','z'):
                        if abs(dot(frame[key],frame[key])-1)>1e-10:raise ValueError('nonunit public frame')
                    if math.dist(cross(frame['x'],frame['y']),frame['z'])>1e-10:raise ValueError('nonorthogonal public frame')
                    if math.dist(frame['origin'],row['point'])>1e-10:raise ValueError('public frame origin differs')
                lookups=row['length_lookups']
                if [r['fractional_tolerance']for r in lookups]!=[1e-2,1e-4,1e-6,1e-8]or not all(math.isfinite(r['t'])for r in lookups):raise ValueError('missing fractional length queries')
        summaries.append(dict(id=op['id'],samples_per_guide=count,target_classification=value['diagnostics']['target']['tolerance_classification']))

    index=next(i for i,o in enumerate(operations)if o['id']=='spatial')
    op=operations[index];value=results[index]['value'];count=op['diagnostic_length_samples']
    base=op['base'];target=op['target']
    if base!={'type':'line','start':[0,0,0],'end':[10,0,0]}or target['type']!='nurbs'or target['degree']!=3 or target['knots']!=[0,0,0,0,1,1,1,1]or len(target['control_points'])!=4 or any(p['weight']!=1 for p in target['control_points']):raise ValueError('dense witness must be an authored cubic Bezier')
    expected=[[10*i/(count-1),y,z]for i in range(count)for y,z in [(0,0),(1,0),(0,1)]]
    if maximum_delta(op['points'],expected)>1e-14:raise ValueError('dense witness basis differs')
    points=value['sdk_points'];rows=value['diagnostics']['target']['length_rows']
    centers=points[::3];ys=[sub(y,p)for y,p in zip(points[1::3],centers)];zs=[sub(z,p)for z,p in zip(points[2::3],centers)]
    if any(abs(math.hypot(*axis)-1)>1e-10 for axis in ys+zs)or any(abs(dot(y,z))>1e-10 for y,z in zip(ys,zs)):raise ValueError('dense native basis is not orthonormal')
    ys=list(map(unit,ys));zs=list(map(unit,zs))
    tangents=[unit(cross(y,z))for y,z in zip(ys,zs)]
    residuals={}
    for name,frames in [('single',[r['perp']for r in rows]),('batch',value['diagnostics']['target']['length_batch'])]:
        angles=[math.atan2(dot(cross(f['x'],y),t),dot(f['x'],y))for f,y,t in zip(frames,ys,tangents)]
        residuals[name]=dict(initial_roll_degrees=math.degrees(angles[0]),max_roll_after_initial_alignment_degrees=max(abs(math.degrees(wrap(a-angles[0])))for a in angles))
    control=[p['point']for p in target['control_points']]
    def bezier(t):
        b=[(1-t)**3,3*(1-t)**2*t,3*(1-t)*t*t,t**3]
        return [sum(w*p[j]for w,p in zip(b,control))for j in range(3)]
    lookup_errors=[]
    for i,error in enumerate([1e-2,1e-4,1e-6,1e-8]):
        lookup_errors.append(dict(fractional_tolerance=error,max_coordinate_error=maximum_delta(centers,[bezier(r['length_lookups'][i]['t'])for r in rows])))
    scale_frames=[]
    for label in ('scale-0-01','scale-0-1'):
        i=next(i for i,o in enumerate(operations)if o['id']==label);o=operations[i];v=results[i]['value'];diag=v['diagnostics']['target']
        plane=next(r['plane']for r in diag['tolerance_classification']if r['tolerance']==.1)
        if plane is None:raise ValueError('scale witness no longer has a public tolerance plane')
        normal=plane['z'];tangent=diag['length_rows'][0]['tangent']
        source_x=unit(sub(o['base']['end'],o['base']['start']));source_y=unit(cross(normal,source_x));source_z=cross(source_x,source_y)
        target_y=unit(cross(normal,tangent));target_z=cross(tangent,target_y)
        scale=o['base']['end'][0]/10
        # Only the exact initial position is compared here. The native plane is
        # evidence for a candidate rule; it is never a kernel construction input.
        axes=[]
        for direction in ([0,1,0],[0,0,1]):
            axes.append([dot(direction,source_y)*y+dot(direction,source_z)*z for y,z in zip(target_y,target_z)])
        actual=[[x/scale for x in sub(v['sdk_points'][i],v['sdk_points'][4])]for i in (5,6)]
        scale_frames.append(dict(id=label,classification_tolerance=.1,max_initial_basis_error=maximum_delta(actual,axes)))
    return dict(cases=len(operations),native_spatial_parity_qualified=False,native_outputs_used_as_kernel_inputs=False,
                dense=dict(samples=count,max_center_difference_from_precise_normalized_length=maximum_delta(centers,[r['point']for r in rows]),public_frame_comparisons=residuals,public_fractional_length_comparisons=lookup_errors),
                initial_scale_frame_candidates=scale_frames,queries=summaries)


def main():
    p=argparse.ArgumentParser(description=__doc__)
    for name in ('request','native','output'):p.add_argument(name,type=Path)
    args=p.parse_args();result=report(json.loads(args.request.read_text()),json.loads(args.native.read_text()))
    args.output.write_text(json.dumps(result,indent=2,allow_nan=False)+'\n')


if __name__=='__main__':main()

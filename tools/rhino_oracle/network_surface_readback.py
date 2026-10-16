"""Build independent Rust closest-point queries on saved Rhino network tensors."""
import argparse
import json
import math
from pathlib import Path

DEFAULT_CASES=('coons_arches','biquadratic_network','rational_cylinder')

def request_from_observations(request,reference,cases=DEFAULT_CASES):
    if reference.get('engine')!='rhino':raise ValueError('expected saved Rhino observations')
    rows={r['id']:r['value']for r in reference['results']}
    if len(rows)!=len(reference['results']):raise ValueError('duplicate observation ids')
    operations=[]
    for op in request['operations']:
        if op['id'] not in cases:continue
        value=rows[op['id']];queries=op['queries'];points=value['closest_points'];d=value['definition']
        if not queries or len(points)!=len(queries):raise ValueError('query count mismatch')
        index=max(range(len(queries)),key=lambda i:math.dist(queries[i],points[i]))
        surface=dict(degree_u=d['degree'][0],degree_v=d['degree'][1],
                     control_point_count_u=d['control_count'][0],control_point_count_v=d['control_count'][1],
                     control_points=d['control_points'],knots_u=d['knots_u'],knots_v=d['knots_v'],
                     domain_u=d['domain_u'],domain_v=d['domain_v'])
        operations.append(dict(op='surface_closest_point',id=op['id'],surface=surface,points=[queries[index]]))
    if len(operations)!=len(set(cases)):raise ValueError('missing requested case')
    return dict(protocol_version=request['protocol_version'],iterations=1,tolerance=request['tolerance'],operations=operations)

def source_distances_from_observations(request,reference):
    """Read back the worst returned source-point distance per original curve.

    These reference-derived tensors diagnose closest-point agreement only;
    they are never independent candidate-network inputs.
    """
    if reference.get('engine')!='rhino':raise ValueError('expected saved Rhino observations')
    results=reference['results']
    if len(results)!=len(request['operations']):raise ValueError('operation count mismatch')
    operations=[]
    for op,row in zip(request['operations'],results):
        if op['id']!=row['id'] or op['op']!='network_surface':raise ValueError('network identity mismatch')
        samples=row['value']['source_samples'];fractions=op.get('source_sample_fractions',[])
        if not fractions or len(samples)!=len(op['curves'])*len(fractions):raise ValueError('source sample count mismatch')
        points=[]
        for index in range(len(op['curves'])):
            group=samples[index*len(fractions):(index+1)*len(fractions)]
            if any(s['source_curve_index']!=index or s['source_fraction']!=fraction
                   or not math.isfinite(s['distance']) or s['distance']<0
                   for s,fraction in zip(group,fractions)):
                raise ValueError('source sample identity or distance mismatch')
            points.append(max(group,key=lambda s:s['distance'])['source_point'])
        d=row['value']['definition']
        surface=dict(degree_u=d['degree'][0],degree_v=d['degree'][1],
            control_point_count_u=d['control_count'][0],control_point_count_v=d['control_count'][1],
            control_points=d['control_points'],knots_u=d['knots_u'],knots_v=d['knots_v'],
            domain_u=d['domain_u'],domain_v=d['domain_v'])
        operations.append(dict(op='surface_closest_point',id=op['id'],surface=surface,points=points))
    return dict(protocol_version=request['protocol_version'],iterations=1,tolerance=request['tolerance'],operations=operations)

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('request',type=Path);parser.add_argument('observations',type=Path);parser.add_argument('output',type=Path)
    parser.add_argument('--source-distances',action='store_true',help='query the worst sampled source distance per curve on reference-derived tensors')
    args=parser.parse_args()
    builder=source_distances_from_observations if args.source_distances else request_from_observations
    result=builder(json.loads(args.request.read_text()),json.loads(args.observations.read_text()))
    args.output.write_text(json.dumps(result,indent=2,allow_nan=False)+'\n')

if __name__=='__main__':main()

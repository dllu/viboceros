"""Compare default graph outlines; retain setting failures and sampling mismatches."""
import argparse,copy,json,math
from pathlib import Path
if __package__:
    from .extract_curvature_graph_comparison import vertex_probe
else:
    from extract_curvature_graph_comparison import vertex_probe
BASE=Path(__file__).parent
CAPTURES=('curvature_sampling','curvature_sampling_controls','curvature_sampling_holdout')
def read(path):return json.loads(path.read_text())
def cases():
    result=[]
    for capture in CAPTURES:
        request=read(BASE/'fixtures'/(capture+'.json'))
        native_name='curvature_sampling_outlines'if capture=='curvature_sampling'else capture
        native=read(BASE/'observations'/(native_name+'_rhino_20261016.json'))
        if len(request['operations'])!=len(native['results']):raise ValueError('capture length mismatch')
        for op,row in zip(request['operations'],native['results']):
            if op['id']!=row['id']:raise ValueError('capture identity mismatch')
            result.append((capture+'/'+op['id'],op,row['value']))
    return result
def local_request():
    # Only original recipe geometry enters the local producer. Native graph
    # vertices, recovered parameters and returned NURBS definitions never do.
    operations=[]
    for id,op,_ in cases():
        operations.append(dict(op='curvature_graph',id=id,source=copy.deepcopy(op['source']),scale=op['scale'],density=op['density']))
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-8),operations=operations)
def sequence_error(first,second):
    if len(first)!=len(second):return None
    def error(a,b):
        if len(a['points'])!=len(b['points']):return None
        return min(max((math.dist(p,q)for p,q in zip(a['points'],points)),default=0.)for points in (b['points'],list(reversed(b['points']))))
    choices=[[error(a,b)for b in second]for a in first]
    # A bottleneck assignment accommodates native object order and reversal.
    thresholds=sorted({0.}|{e for row in choices for e in row if e is not None})
    for threshold in thresholds:
        owners={}
        def visit(i,seen):
            for j,e in enumerate(choices[i]):
                if e is None or e>threshold or j in seen:continue
                seen.add(j)
                if j not in owners or visit(owners[j],seen):owners[j]=i;return True
            return False
        if all(visit(i,set())for i in range(len(first))):return threshold
    return None
def comparison(local):
    expected=cases()
    if len(local['results'])!=len(expected):raise ValueError('local case length mismatch')
    rows=[]
    for (id,op,native),row in zip(expected,local['results']):
        if id!=row['id']:raise ValueError('local identity mismatch')
        value=row['value'];settings=dict(scale=op['scale'],density=op['density'])
        if value['settings']!=settings or any(native['settings'].get(k)!=v for k,v in settings.items()):raise ValueError('graph settings mismatch')
        first,second=value['outputs'],native['outputs']
        if any(len(obj.get('points',[]))<2 for obj in first+second)or any(len(p)!=3 or not all(math.isfinite(c)for c in p)for obj in first+second for p in obj['points']):raise ValueError('invalid outline point')
        error=sequence_error(first,second)
        applied='sample_density'not in op or native['settings'].get('sample_density')==op['sample_density']
        rows.append(dict(id=id,requested_settings_applied=applied,requested_sample_density=op.get('sample_density',2),observed_sample_density=native['settings'].get('sample_density',2),sources_unchanged=value['inputs_unchanged']and native['inputs_unchanged'],vertex_counts=[[len(obj['points'])for obj in outputs]for outputs in (first,second)],unordered_reversible_vertex_sequence_error=error,vertex_sequences_within_1e_minus_6=applied and error is not None and error<=1e-6,vertex_to_polyline_distances=[vertex_probe(first,second),vertex_probe(second,first)],displayed_hairs=value['hair_count'],outline_samples=value['outline_sample_count'],maximum_native_station_recovery_residual=max((s['residual']for g in native['graphs']for s in g['stations']),default=0.)))
    return dict(schema_version=1,cases=len(rows),settings_applied_cases=sum(r['requested_settings_applied']for r in rows),matched_vertex_sequence_cases=sum(r['vertex_sequences_within_1e_minus_6']for r in rows),unchanged_source_cases=sum(r['sources_unchanged']for r in rows),complete_native_graph_parity_qualified=False,native_hair_placement_qualified=False,continuous_source_curvature_qualified=False,performance_qualified=False,method='Compare actual output polylines at 1e-6 with a bottleneck assignment of corresponding vertex sequences, allowing object reordering and orientation reversal. Retain bidirectional vertex-to-polyline diagnostics for different grids. Native station recovery is diagnostic and may select a wrong branch on looping curves; no recovered station is used as a producer input. Requested settings that Rhino did not apply cannot qualify a match.',results=rows)
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--prepare',type=Path);p.add_argument('--local',type=Path);p.add_argument('--output',type=Path);a=p.parse_args()
    if a.prepare:a.prepare.write_text(json.dumps(local_request(),indent=2)+'\n')
    if a.local:
        if not a.output:p.error('--local requires --output')
        result=comparison(read(a.local));a.output.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:result[k]for k in ('cases','settings_applied_cases','matched_vertex_sequence_cases','unchanged_source_cases')}))
if __name__=='__main__':main()

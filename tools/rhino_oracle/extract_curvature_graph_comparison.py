"""Compare extraction policies and retain sampled-polyline geometry differences."""
import argparse,json,math
from pathlib import Path
ATTRS=('type','selected','layer','name','color_source','color','group_count','user_text','geometry_user_text')
def point_segment(p,a,b):
    d=[y-x for x,y in zip(a,b)];v=[x-y for x,y in zip(p,a)];length=sum(x*x for x in d)
    t=max(0.,min(1.,sum(x*y for x,y in zip(v,d))/length))if length else 0.
    return math.sqrt(sum((x-t*y)**2 for x,y in zip(v,d)))
def vertex_probe(first,second):
    if not first and not second:return 0.
    if not first or not second:return None
    segments=[(a,b)for o in second for a,b in zip(o['points'],o['points'][1:])]
    if not segments:return None
    return max(min(point_segment(p,a,b)for a,b in segments)for o in first for p in o['points'])
def endpoint_sets(first,second):
    if len(first)!=len(second):return False
    choices=[]
    for a in first:
        a=[a['points'][0],a['points'][-1]]
        choices.append([j for j,b in enumerate(second)if any(all(math.dist(p,q)<=1e-6 for p,q in zip(a,ends))for ends in ([b['points'][0],b['points'][-1]],[b['points'][-1],b['points'][0]]))])
    owners={}
    def visit(i,seen):
        for j in choices[i]:
            if j in seen:continue
            seen.add(j)
            if j not in owners or visit(owners[j],seen):owners[j]=i;return True
        return False
    return all(visit(i,set())for i in range(len(first)))
def comparison(request,local,native):
    if not len(request['operations'])==len(local['results'])==len(native['results']):raise ValueError('extraction case count mismatch')
    rows=[]
    for op,a,b in zip(request['operations'],local['results'],native['results']):
        if not op['id']==a['id']==b['id']:raise ValueError('extraction case identity mismatch')
        x,y=a['value'],b['value'];first,second=x['outputs'],y['outputs']
        if any(not o['points']or any(len(p)!=3 or not all(math.isfinite(c)for c in p)for p in o['points'])for o in first+second):raise ValueError('invalid extraction polyline')
        metadata=len(first)==len(second)and all(all(a[k]==b[k]for k in ATTRS)for a,b in zip(first,second))
        forward,reverse=vertex_probe(first,second),vertex_probe(second,first)
        row=dict(id=op['id'],command_status_matches=x['command_returned']==y['command_returned'],piece_counts=[len(first),len(second)],piece_count_matches=len(first)==len(second),attributes_match=metadata,source_selection_matches=x['sources_selected']==y['sources_selected'],sources_unchanged=x['inputs_unchanged']and y['inputs_unchanged'],outline_endpoint_sets_match=endpoint_sets(first,second),vertex_counts=[[len(o['points'])for o in outputs]for outputs in (first,second)],vertex_to_polyline_distances=[forward,reverse],vertex_probes_within_1e_minus_6=forward is not None and reverse is not None and max(forward,reverse)<=1e-6)
        if 'undo'in x and 'undo'in y:
            row.update(local_undo_removes_outputs=not x['undo']['outputs'],local_redo_restores_outputs=x['redo']['outputs']==first,native_undo_removes_outputs=not y['undo']['outputs'],native_undo_returned=y['undo'].get('command_returned'),native_undo_history=y['undo'].get('command_history'))
        rows.append(row)
    counts={key:sum(bool(r[key])for r in rows)for key in ('command_status_matches','piece_count_matches','attributes_match','source_selection_matches','sources_unchanged','outline_endpoint_sets_match','vertex_probes_within_1e_minus_6')}
    return dict(schema_version=1,cases=len(rows),counts=counts,complete_native_command_parity_qualified=False,native_polyline_geometry_qualified=False,native_undo_redo_qualified=False,continuous_geometry_distance_qualified=False,performance_qualified=False,method='Actual command status, attributes, selection and piece counts; endpoint sets are orientation independent at 1e-6. Bidirectional vertex-to-other-polyline distances expose sampling differences and are not continuous Hausdorff certificates. All mismatches remain. Native nested-script Undo reports Nothing to undo despite a true command return; local model undo is tested separately.',results=rows)
def main():
    p=argparse.ArgumentParser(description=__doc__)
    for name in ('request','local','native','output'):p.add_argument(name,type=Path)
    args=p.parse_args();result=comparison(*[json.loads(f.read_text())for f in (args.request,args.local,args.native)]);args.output.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result['counts']))
if __name__=='__main__':main()

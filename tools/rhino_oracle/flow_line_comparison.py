"""Compare Flow Line guide lifetime, complete owners and real Undo/Redo."""
import argparse
import json
from pathlib import Path
from .orient_surface_grips_comparison import difference


def comparison(request,local,native):
    def rows(response):
        r={v['id']:v['value']for v in response['results']}
        if len(r)!=len(response['results']):raise ValueError('duplicate Flow Line result')
        return r
    aa,nn=rows(local),rows(native);ids=[o['id']for o in request['operations']];recipes={o['id']:o for o in request['operations']}
    if len(ids)!=len(set(ids))or set(ids)!=set(aa)or set(aa)!=set(nn):raise ValueError('Flow Line identities differ')
    if not request['protocol_version']==local['protocol_version']==native['protocol_version']:raise ValueError('Flow Line protocol differs')
    results=[]
    for id in ids:
        a,n=aa[id],nn[id]
        if recipes[id]['op']=='flow':
            valid,error=difference(a['sdk_points'],n['sdk_points'])
            results.append(dict(id=id,within_1e_minus_6=bool(valid and error<=1e-6),stages=[dict(stage='sdk',native_stage='sdk',metadata_match=bool(valid),max_coordinate_error=error,within_1e_minus_6=bool(valid and error<=1e-6))]))
            continue
        good=a.get('command_returned')is True and n.get('command_returned')is True
        good &= a.get('undo_restores_source')is True and a.get('redo_restores_outputs')is True
        good &= n.get('undo_command_returned')is True and n.get('redo_command_returned')is True and 'Unknown command'not in n.get('history','')
        stages=[]
        for stage in ('before','after','undo','redo'):
            native_stage='after_script'if stage=='after'else stage;x,y=a[stage],n[native_stage]
            metadata=all(x[k]==y[k]for k in ('object_count','group_count','guide_selected','guide_unchanged'))
            metadata &= x['guide_unchanged']is True and x['guide_selected']is False
            metadata &= len(x['outputs'])==len(y['outputs']) and bool(x['outputs'])
            unused=list(y['outputs']);error=0.
            for o in x['outputs']:
                options=[(not ok,e,i)for i,r in enumerate(unused)for ok,e in[difference(o,r)]]
                if not options:metadata=False;continue
                bad,e,i=min(options);unused.pop(i);metadata &= not bad;error=max(error,e)
            stages.append(dict(stage=stage,native_stage=native_stage,metadata_match=bool(metadata),max_coordinate_error=error,within_1e_minus_6=bool(metadata and error<=1e-6)))
            good &= metadata and error<=1e-6
        results.append(dict(id=id,within_1e_minus_6=bool(good),stages=stages))
    return dict(cases=len(results),matched_cases=sum(r['within_1e_minus_6']for r in results),native_table_enumeration_order_compared=False,results=results)


def main():
    p=argparse.ArgumentParser(description=__doc__)
    for key in ('request','local','native','output'):p.add_argument(key,type=Path)
    args=p.parse_args();read=lambda path:json.loads(path.read_text())
    r=comparison(read(args.request),read(args.local),read(args.native))
    args.output.write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    if r['matched_cases']!=r['cases']:raise SystemExit('Flow Line comparisons failed; see report')


if __name__=='__main__':main()

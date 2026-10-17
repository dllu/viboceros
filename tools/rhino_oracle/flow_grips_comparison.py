"""Compare Flow owner definitions, selected grips, group topology and history."""
import argparse
import json
from pathlib import Path
from .orient_surface_grips_comparison import difference


def comparison(request, local, native):
    def rows(response):
        rows={r['id']:r['value']for r in response['results']}
        if len(rows)!=len(response['results']):raise ValueError('duplicate Flow grip result')
        return rows
    a,b=rows(local),rows(native);ids=[o['id']for o in request['operations']]
    if len(ids)!=len(set(ids))or set(ids)!=set(a)or set(a)!=set(b):raise ValueError('Flow grip case identities differ')
    if not request['protocol_version']==local['protocol_version']==native['protocol_version']:raise ValueError('Flow grip protocol differs')
    result=[]
    for id in ids:
        x,y=a[id],b[id];good=x.get('command_returned')is True and y.get('command_returned')is True
        good &= x.get('undo_restores_source')is True and x.get('redo_restores_outputs')is True
        good &= y.get('undo_command_returned')is True and y.get('redo_command_returned')is True
        good &= 'Unknown command'not in y.get('history','')
        stages=[]
        for stage in ('before','after','undo','redo'):
            native_stage='after_script'if stage=='after'else stage
            aa,nn=x[stage],y[native_stage]
            metadata=aa['group_count']==nn['group_count'] and len(aa['outputs'])==len(nn['outputs']) and bool(aa['outputs'])
            metadata &= aa['guides_unchanged']is True and nn['guides_unchanged']is True
            metadata &= aa['guides_selected']==nn['guides_selected']==[False,False]
            unused=list(nn['outputs']);error=0.
            for output in aa['outputs']:
                candidates=[(not valid,delta,i)for i,n in enumerate(unused)for valid,delta in[difference(output,n)]]
                if not candidates:metadata=False;continue
                bad,delta,i=min(candidates);unused.pop(i);metadata &= not bad;error=max(error,delta)
            good &= metadata and error<=1e-6
            stages.append(dict(stage=stage,native_stage=native_stage,metadata_match=bool(metadata),max_coordinate_error=error,within_1e_minus_6=bool(metadata and error<=1e-6)))
        result.append(dict(id=id,within_1e_minus_6=bool(good),stages=stages))
    return dict(cases=len(result),matched_cases=sum(r['within_1e_minus_6']for r in result),native_table_enumeration_order_compared=False,results=result)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ('request','local','native','output'):parser.add_argument(name,type=Path)
    args=parser.parse_args();read=lambda p:json.loads(p.read_text())
    result=comparison(read(args.request),read(args.local),read(args.native))
    args.output.write_text(json.dumps(result,indent=2,allow_nan=False)+'\n')
    if result['matched_cases']!=result['cases']:raise SystemExit('Flow grip comparisons failed; see report')


if __name__=='__main__':main()

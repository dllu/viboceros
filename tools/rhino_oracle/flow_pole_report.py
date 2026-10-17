"""Retain qualified Roadlike pole workflows and unresolved native length lookups."""
import argparse,json,math
from pathlib import Path
from .flow_line_comparison import comparison
BASE=Path(__file__).parent


def report(base=BASE):
    def read(path):return json.loads((base/path).read_text())
    workflows=[]
    for name in ('flow_roadlike_diagnostics','flow_roadlike_poles'):
        q=read('fixtures/'+name+'.json');a=read('observations/'+name+'_viboceros_20261016.json');n=read('observations/'+name+'_rhino_20261016.json')
        r=comparison(q,a,n)
        rejected=[]
        for op,x,y,row in zip(q['operations'],a['results'],n['results'],r['results']):
            if row['within_1e_minus_6']:continue
            v=x['value'];atomic=v['command_returned']is False and v['undo_label']is None and v['redo_label']is None and all(v['before']==v[k]for k in ('after','undo','redo'))
            rejected.append(dict(id=op['id'],native_command_succeeded=y['value']['command_returned']is True,local_rejection_atomic=atomic,error=v['error']))
        workflows.append(dict(fixture=name,cases=r['cases'],qualified_matches=r['matched_cases'],rejected=rejected,max_qualified_coordinate_error=max((s['max_coordinate_error']for row in r['results']if row['within_1e_minus_6']for s in row['stages']),default=0.)))
    name='flow_pole_lookup';q=read('fixtures/'+name+'.json');a=read('observations/'+name+'_viboceros_20261016.json');n=read('observations/'+name+'_rhino_20261016.json');lookups=[]
    if not q['protocol_version']==a['protocol_version']==n['protocol_version']or [o['id']for o in q['operations']]!=[x['id']for x in a['results']]or [x['id']for x in a['results']]!=[x['id']for x in n['results']]:raise ValueError('lookup identities/protocol differ')
    for op,x,y in zip(q['operations'],a['results'],n['results']):
        c=op['target'];degree=c['degree'];controls=c['control_points']
        if c['type']!='nurbs'or len(controls)!=degree+1 or c['knots']!=[0]*(degree+1)+[1]*(degree+1)or op['base']!=dict(type='line',start=[0,0,0],end=[10,0,0]):raise ValueError('lookup witness must be a Bezier and a ten-unit X source')
        aa=x['value']['sdk_points'];nn=y['value']['sdk_points']
        if len(aa)!=len(nn)or len(nn)!=len(op['points'])or any(len(p)!=3 or any(not math.isfinite(v)for v in p)for p in aa+nn):raise ValueError('invalid lookup points')
        error=max(abs(a-b)for p,r in zip(aa,nn)for a,b in zip(p,r))
        batch=next(b for b in y['value']['diagnostics']['target']['length_batches']if b['divisions']==840 and b['fractional_tolerance']==1e-4)
        parameters=batch['parameters']
        if len(parameters)!=841 or parameters[0]!=0 or parameters[-1]!=1 or any(not math.isfinite(t)for t in parameters)or any(a>=b for a,b in zip(parameters,parameters[1:])):raise ValueError('invalid native diagnostic inverse table')
        prediction=[]
        for p,actual in zip(op['points'],nn):
            if p[1:]!=[0,0]:raise ValueError('lookup witnesses must lie on source guide')
            fraction=p[0]/10
            if not 0<=fraction<=1:continue
            index=min(int(fraction*840),839);weight=fraction*840-index;t=(1-weight)*parameters[index]+weight*parameters[index+1]
            basis=[math.comb(degree,i)*(1-t)**(degree-i)*t**i*c['weight']for i,c in enumerate(controls)];denominator=sum(basis)
            point=[sum(b*c['point'][axis]for b,c in zip(basis,controls))/denominator for axis in range(3)]
            prediction.append(max(abs(a-b)for a,b in zip(point,actual)))
        lookups.append(dict(id=op['id'],precise_kernel_max_coordinate_error=error,within_document_tolerance=error<=q['tolerance']['absolute'],public_840_prediction_samples=len(prediction),public_840_prediction_max_coordinate_error=max(prediction)))
    return dict(native_commands=sum(r['cases']for r in workflows),qualified_commands=sum(r['qualified_matches']for r in workflows),workflows=workflows,lookup_diagnostics=lookups,native_values_used_as_kernel_inputs=False,complete_flow_qualified=False)


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('output',type=Path);args=parser.parse_args()
    args.output.write_text(json.dumps(report(),indent=2,allow_nan=False)+'\n')


if __name__=='__main__':main()

"""Complementary and central symmetric picks, including numeric parameters."""
import copy,hashlib,json
from pathlib import Path
from .generate_insert_knot_command import ROOT,request as initial_request,capture
def request():
    q=initial_request();templates={}
    for op in q['operations']:templates.setdefault(op['family'],op)
    folder=ROOT/'tools/rhino_oracle/fixtures/insert_knot_symmetry';folder.mkdir(exist_ok=True);out=[]
    for family in('cubic','rational','circle','periodic','polycurve','surface','surface_rational'):
        surface=family.startswith('surface')
        actions=[('complement',[dict(point=[.35,.6]if surface else .35),dict(point=[.65,.4]if surface else .65)]),('center',[dict(point=[.5,.5]if surface else .5)])]
        if not surface:actions.append(('parameters',[dict(parameter=.35),dict(parameter=.65)]))
        for suffix,steps in actions:
            op=copy.deepcopy(templates[family]);name=family+'_symmetric_'+suffix;op.update(id=name,actions=steps,symmetrical=True,artifact_path=str(folder/(name+'.3dm')))
            if surface:op['direction']='Both'
            out.append(op)
    q['operations']=out;return q
def main():
    q=request();folder=ROOT/'tools/rhino_oracle/observations';(folder/'insert_knot_symmetry.request.json').write_text(json.dumps(q,indent=2)+'\n')
    destination=folder/'insert_knot_symmetry.json';capture(q,destination)
    data=json.loads(destination.read_text());data['provenance']['sources'][Path(__file__).name]=hashlib.sha256(Path(__file__).read_bytes()).hexdigest();destination.write_text(json.dumps(data,indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()

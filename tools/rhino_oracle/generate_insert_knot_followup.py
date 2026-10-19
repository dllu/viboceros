"""Nonuniform Automatic, local Undo groups, guides, and a trimmed-hole pick."""
import copy,hashlib,json
from pathlib import Path
from .generate_insert_knot_command import ROOT,request as initial_request,capture
def request():
    q=initial_request();templates={}
    for op in q['operations']:templates.setdefault(op['family'],op)
    folder=ROOT/'tools/rhino_oracle/fixtures/insert_knot_followup';folder.mkdir(exist_ok=True)
    sequences=[('point_auto',[dict(point=.35),'Automatic']),('auto_undo',['Automatic','Undo']),('auto_twice_undo',['Automatic','Automatic','Undo']),('point_auto_undo',[dict(point=.35),'Automatic','Undo']),('symmetric_auto',['Symmetrical=Yes',dict(point=.35),'Automatic']),('midpoint_guide',['Midpoints=Yes',dict(point=.35)]),('undo_all',[dict(point=.35),'Automatic','Undo','Undo']),('symmetric_undo',['Symmetrical=Yes',dict(point=.35),dict(point=.7),'Undo'])]
    out=[]
    for family in('cubic','rational','periodic','polycurve','surface','surface_rational'):
        for suffix,actions in sequences:
            op=copy.deepcopy(templates[family]);name=family+'_'+suffix;op.update(id=name,actions=actions,artifact_path=str(folder/(name+'.3dm')))
            if family.startswith('surface'):op['direction']='Both'
            out.append(op)
    op=copy.deepcopy(templates['trimmed_hole']);name='trimmed_hole_center';op.update(id=name,actions=[dict(point=[.5,.5])],direction='Both',artifact_path=str(folder/(name+'.3dm')));out.append(op)
    q['operations']=out;return q
def main():
    q=request();folder=ROOT/'tools/rhino_oracle/observations';(folder/'insert_knot_followup.request.json').write_text(json.dumps(q,indent=2)+'\n')
    destination=folder/'insert_knot_followup.json';capture(q,destination)
    data=json.loads(destination.read_text());data['provenance']['sources'][Path(__file__).name]=hashlib.sha256(Path(__file__).read_bytes()).hexdigest();destination.write_text(json.dumps(data,indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()

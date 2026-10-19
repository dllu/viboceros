"""Capture analytic and generic NURBS Weight representations at repeated knots."""
import copy
import hashlib
import json
import subprocess
from pathlib import Path
from .generate_grip_weight import request
from .client import OracleClient
from .structural_sdk import validate_operations


def main():
    h=Path(__file__).resolve().parent;folder=h/'fixtures/grip_weight_representations';folder.mkdir(exist_ok=True)
    base=request()['operations'][0];operations=[]
    for kind in('circle','arc'):
        for nurbs in(False,True):
            for seed in('last','second','adjacent','before_last','ends'):
                op=copy.deepcopy(base);name=kind+('_nurbs_'if nurbs else'_analytic_')+seed
                op.update(op='grip_weight_nurbs'if nurbs else'grip_weight',id=name,artifact_path=str(folder/(name+'.3dm')))
                op['sources'][0].update(kind=kind,selected=seed);operations.append(op)
    q=dict(protocol_version=1,iterations=1,operations=operations);validate_operations(q)
    path=h/'fixtures/grip_weight_representations.json';path.write_text(json.dumps(q,indent=2)+'\n')
    r=OracleClient(settings_scheme='VibocerosOracleWeightRepresentations20261019').run_rhino(q,timeout=600.)
    assert [o['id']for o in q['operations']]==[o['id']for o in r['results']]
    names=('generate_weight_representations.py','generate_grip_weight.py','weight_nurbs_probe.py','grip_weight_probe.py','weight_dialog.py','delete_grips_probe.py','grip_navigation_probe.py','grip_transform_probe.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py')
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),request_sha256=digest(path),sources={n:digest(h/n)for n in names},fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path']))for o in operations})
    diagnostics={o['id']:json.loads(Path(o['artifact_path']+'.weight.log').read_text())for o in operations}
    path=h/'observations/grip_weight_representations_input.json';path.write_text(json.dumps(diagnostics,indent=2)+'\n');r['provenance']['diagnostics_sha256']=digest(path)
    (h/'observations/grip_weight_representations.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    for o in operations:
        Path(o['artifact_path']+'.capture.json').unlink();Path(o['artifact_path']+'.weight.log').unlink()
    print('Recorded',len(operations),'representation workflows',flush=True)


if __name__=='__main__':main()

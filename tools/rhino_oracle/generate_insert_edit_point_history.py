"""Capture redo preservation/replacement for empty and coincident edit input."""
import hashlib,json,subprocess
from pathlib import Path
from .client import OracleClient
from .structural_sdk import validate_operations


def request():
    ops=[]
    for text in(False,True):
        for pre in(False,True):
            for station in('empty','existing','new'):
                for cancel in(False,True):
                    ops.append(dict(op='insert_edit_point_history',id='history_'+str(len(ops)),station=station,cancel=cancel,preselect=pre,geometry_text=text))
    q=dict(protocol_version=1,iterations=1,operations=ops);validate_operations(q);return q


def main():
    h=Path(__file__).resolve().parent;q=request();p=h/'fixtures/insert_edit_point_history.json';p.write_text(json.dumps(q,indent=2)+'\n')
    r=OracleClient(settings_scheme='VibocerosOracleInsertEditPointHistory20261019').run_rhino(q,timeout=600.)
    assert [o['id']for o in r['results']]==[o['id']for o in q['operations']]
    assert all('error'not in o for o in r['results'])
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    names=('generate_insert_edit_point_history.py','insert_edit_point_history_probe.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py')
    r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),request_sha256=digest(p),sources={n:digest(h/n)for n in names})
    (h/'observations/insert_edit_point_history.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    print('Recorded',len(r['results']),'history workflows',flush=True)


if __name__=='__main__':main()

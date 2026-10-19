"""Capture ordinary selection on empty Undo independently of editing commands."""
import hashlib,json,subprocess
from pathlib import Path
from .client import OracleClient
from .structural_sdk import validate_operations


def main():
    h=Path(__file__).resolve().parent
    q=dict(protocol_version=1,iterations=1,operations=[dict(op='empty_undo_selection',id=kind+('_redo'if redo else'_empty'),kind=kind,redo=redo)for kind in('point','curve')for redo in(False,True)])
    validate_operations(q);p=h/'fixtures/empty_undo_selection.json';p.write_text(json.dumps(q,indent=2)+'\n')
    r=OracleClient(settings_scheme='VibocerosOracleEmptyUndoSelection20261019').run_rhino(q,timeout=600.)
    assert len(r['results'])==4 and all('error'not in o for o in r['results'])
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    names=('generate_empty_undo_selection.py','empty_undo_selection_probe.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py')
    r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),request_sha256=digest(p),sources={n:digest(h/n)for n in names})
    (h/'observations/empty_undo_selection.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n');print('Recorded four empty Undo selection workflows',flush=True)


if __name__=='__main__':main()

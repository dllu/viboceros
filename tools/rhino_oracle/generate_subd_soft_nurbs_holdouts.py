"""Native fractional and variable-end sharpness inputs from frozen public edits."""
import base64
import hashlib
import json
from pathlib import Path
from .capture_family import capture
from .subd_soft_nurbs_cases import holdout_cases

HERE=Path(__file__).resolve().parent
SOURCES={
    'variable_both_ends':('soft_crease_numeric_labels','label_both_ends'),
    'variable_chain':('soft_crease_chains','chains_seed_50_end'),
    'variable_branch':('soft_crease_numeric_labels','label_branch'),
    'hard_soft_chain':('soft_crease_chains','chains_partial_hard_middle'),
}

def inputs():
    folder=HERE/'fixtures/subd_soft_nurbs_inputs';folder.mkdir(exist_ok=True)
    provenance={}
    for name,(family,key) in SOURCES.items():
        path=HERE/'observations'/(family+'.json');data=json.loads(path.read_bytes())
        row=next(r for r in data['results'] if r['id']==key)
        raw=base64.b64decode(row['value']['result_3dm']);asset=folder/(name+'.3dm')
        if asset.exists() and asset.read_bytes()!=raw:raise ValueError('native input archive changed')
        asset.write_bytes(raw)
        provenance[name]=dict(family=family,case=key,source_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),archive_sha256=hashlib.sha256(raw).hexdigest())
    manifest=folder/'provenance.json';encoded=(json.dumps(provenance,indent=2)+'\n').encode()
    if manifest.exists() and manifest.read_bytes()!=encoded:raise ValueError('native input provenance changed')
    manifest.write_bytes(encoded)

if __name__=='__main__':
    inputs()
    query=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=holdout_cases())
    capture('subd_soft_nurbs_holdouts',query,['generate_subd_soft_nurbs_holdouts.py','subd_soft_nurbs_cases.py','subd_soft_nurbs_probe.py',
        'subd_packing_cases.py','subd_g1x_probe.py','subd_continuity_probe.py','subd_g0_probe.py','subd_to_nurbs_probe.py',
        'subd_join_probe.py','subd_join_cases.py','subd_box_probe.py','join_probe.py','client.py','rhino_worker.py',
        'structural_sdk.py','merge_edges_probe.py','fixtures/subd_soft_nurbs_inputs/provenance.json',
        *['fixtures/subd_soft_nurbs_inputs/'+name+'.3dm' for name in SOURCES]],
        'VibocerosOracleSubDSoftNurbsHoldouts20261104',timeout=2400)

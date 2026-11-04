"""Native uniform, closed and n-gon soft-crease chain conversion controls."""
import base64
import hashlib
import json
from pathlib import Path
from .capture_family import capture
from .subd_soft_nurbs_cases import chain_cases

HERE=Path(__file__).resolve().parent
SOURCES={
    'constant_chain':('soft_crease_chains','chains_multiple_constant'),
    'variable_loop':('soft_crease_numeric_labels','label_loop'),
    'variable_ngon_loop':('soft_crease_chains','chains_odd_loop_50_vertex_0'),
}

def inputs():
    folder=HERE/'fixtures/subd_soft_nurbs_inputs';provenance={}
    for name,(family,key) in SOURCES.items():
        source=HERE/'observations'/(family+'.json');data=json.loads(source.read_bytes())
        row=next(r for r in data['results'] if r['id']==key)
        if row['value']['success'] is not True:raise ValueError('native source command failed')
        raw=base64.b64decode(row['value']['result_3dm']);asset=folder/(name+'.3dm')
        if asset.exists() and asset.read_bytes()!=raw:raise ValueError('native input archive changed')
        asset.write_bytes(raw)
        provenance[name]=dict(family=family,case=key,source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),archive_sha256=hashlib.sha256(raw).hexdigest())
    manifest=folder/'chain_provenance.json';encoded=(json.dumps(provenance,indent=2)+'\n').encode()
    if manifest.exists() and manifest.read_bytes()!=encoded:raise ValueError('native input provenance changed')
    manifest.write_bytes(encoded)

if __name__=='__main__':
    inputs()
    query=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=chain_cases())
    capture('subd_soft_nurbs_chains',query,['generate_subd_soft_nurbs_chains.py','subd_soft_nurbs_cases.py','subd_soft_nurbs_probe.py',
        'subd_packing_cases.py','subd_g1x_probe.py','subd_continuity_probe.py','subd_g0_probe.py','subd_to_nurbs_probe.py',
        'subd_join_probe.py','subd_join_cases.py','subd_box_probe.py','join_probe.py','client.py','rhino_worker.py',
        'structural_sdk.py','merge_edges_probe.py','fixtures/subd_soft_nurbs_inputs/chain_provenance.json',
        *['fixtures/subd_soft_nurbs_inputs/'+name+'.3dm' for name in SOURCES]],
        'VibocerosOracleSubDSoftNurbsChains20261104',timeout=2400)

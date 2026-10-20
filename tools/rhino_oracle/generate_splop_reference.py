"""Retain reference-sphere modes using full public Splop point-cloud outputs."""
import hashlib,json
from pathlib import Path
from .client import OracleClient
from .splop_reference_probe import RECIPES
from .structural_sdk import validate_operations
HERE=Path(__file__).resolve().parent


def request():
    ops=[dict(op='splop_reference',id='sphere_'+mode,mode=mode,plane='WorldXY')for mode in RECIPES]
    ops += [dict(op='splop_reference',id='sphere_'+mode+'_ZX',mode=mode,plane='WorldZX')for mode in ('Radius','TwoY','ThreeXY','FourSection')]
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=ops)
    validate_operations(q);return q


def main():
    q=request();p=HERE/'fixtures/splop_reference.json';p.write_text(json.dumps(q,indent=2)+'\n')
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    names=['generate_splop_reference.py','splop_reference_probe.py','splop_probe.py','twist_command_probe.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py']
    sources={n:digest(HERE/n)for n in names}
    r=OracleClient(settings_scheme='VibocerosOracleSplopReference20261020').run_rhino(q,timeout=600.)
    assert sources=={n:digest(HERE/n)for n in names}
    assert len(r['results'])==len(q['operations'])and all('error'not in row for row in r['results'])
    r['provenance']=dict(request_sha256=digest(p),sources=sources)
    (HERE/'observations/splop_reference.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    print('Recorded reference spheres',len(r['results']),flush=True)


if __name__=='__main__':main()

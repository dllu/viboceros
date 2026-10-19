"""Measure public SDK default and precise lengths without modifying fixtures."""
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .structural_sdk import validate_operations

HERE=Path(__file__).resolve().parent


def main():
    source=json.loads((HERE/'fixtures/soft_edit_curve_command.json').read_text())
    q=dict(protocol_version=1,iterations=1,tolerance=source['tolerance'],operations=[
        dict(op='soft_edit_curve_lengths',id=o['id'],station=o['stations'][0],artifact_path=o['artifact_path'])for o in source['operations']])
    validate_operations(q);p=HERE/'fixtures/soft_edit_curve_lengths.json';p.write_text(json.dumps(q,indent=2)+'\n')
    r=OracleClient(settings_scheme='VibocerosOracleSoftEditLengths20261019').run_rhino(q,timeout=900.)
    assert len(r['results'])==187 and all('error'not in o for o in r['results'])
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    r['provenance']=dict(request_sha256=digest(p),sources={n:digest(HERE/n)for n in
        ('generate_soft_edit_lengths.py','soft_edit_lengths_probe.py','rhino_worker.py','structural_sdk.py')},
        fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path']))for o in q['operations']})
    (HERE/'observations/soft_edit_curve_lengths.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    print('Recorded',len(r['results']),'independent SDK length diagnostics',flush=True)


if __name__=='__main__':main()

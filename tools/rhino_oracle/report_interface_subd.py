"""Rebuild API and GUI comparisons of native SubD interface state."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
from .references.interface_subd import request
from .qualification_sources import with_shared_sources
from .report_extract_points import compare

ROOT=Path(__file__).resolve().parents[2]
HERE=Path(__file__).resolve().parent
SOURCES=with_shared_sources([
    'crates/viboceros-oracle/src/interface.rs','crates/viboceros-oracle/tests/interface_subd_native.rs',
    'crates/viboceros-command/src/interface.rs','src/app.rs',
    'tools/rhino_oracle/client.py','tools/rhino_oracle/rhino_worker.py',
    'tools/rhino_oracle/interface_commands_probe.py','tools/rhino_oracle/references/interface_subd.py',
    'tools/rhino_oracle/generate_interface_subd.py','tools/rhino_oracle/report_interface_subd.py',
    'tools/rhino_oracle/test_interface_commands.py','tools/rhino_oracle/test_interface_subd.py',
    'tools/rhino_oracle/test_worker.py','docs/oracle/interface-state.md',
])

def native():
    query=HERE/'fixtures/interface_subd.json';observation=HERE/'observations/interface_subd.json'
    payload=json.loads(query.read_text());data=json.loads(observation.read_text())
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    if payload!=request() or digest(query)!=data['provenance']['request_sha256']:
        raise ValueError('Interface SubD request changed')
    paths={str(p.relative_to(ROOT)) for p in [query,observation]}
    for source,sha in data['provenance']['sources'].items():
        path=HERE/'observations/source_snapshots'/(sha+'.gz')
        if hashlib.sha256(gzip.decompress(path.read_bytes())).hexdigest()!=sha:
            raise ValueError('Original interface producer changed: '+source)
        paths.add(str(path.relative_to(ROOT)))
    if [r['id'] for r in data['results']]!=[o['id'] for o in payload['operations']]:
        raise ValueError('Native interface coverage differs')
    for op,row in zip(payload['operations'],data['results']):
        states=row['value']['states'];flat=op['subd_control_net']
        if len(states)!=len(op['commands'])+1:raise ValueError('Native state count differs')
        initial={k:op[k] for k in ['grid_snap','osnap','smart_track','active_viewport','display_modes','subd_control_net']}
        compare(states[0],initial)
        for index,state in enumerate(states):
            if index and op['commands'][index-1].split()[0].lstrip("'_- ").lower()=='subddisplaytoggle':flat=not flat
            if (any(type(state[k]) is not bool for k in ['grid_snap','osnap','smart_track','subd_control_net']) or
                    state['subd_control_net'] is not flat or state['active_viewport']!=op['active_viewport'] or
                    len(state['display_modes'])!=4 or any(m not in ['Wireframe','Shaded','Ghosted'] for m in state['display_modes'])):
                raise ValueError('Native interface state differs')
    return data,{r['id']:r['value'] for r in data['results']},paths

def measured(log,prefix,expected):
    rows=[json.loads(line.split(prefix+' ',1)[1]) for line in log.splitlines() if prefix+' ' in line]
    ids=[r['id'] for r in rows]
    if len(ids)!=len(set(ids)) or set(ids)!=set(expected):raise ValueError('Interface measurement coverage differs')
    for row in rows:compare(row['value'],expected[row['id']])
    return sorted(rows,key=lambda r:r['id'])

def summarize(oracle_log,app_log):
    data,expected,paths=native()
    oracle=measured(oracle_log,'INTERFACE_SUBD_ORACLE',expected)
    app=measured(app_log,'INTERFACE_SUBD_APP',expected)
    return dict(scope='Explicit SubD interface state through the Python protocol, Rust reducer and GUI',
        native_version=data['engine_version'],captured=len(expected),oracle_replayed=len(oracle),app_replayed=len(app),
        states_per_path=sum(len(v['states']) for v in expected.values()),mismatches=0,oracle=oracle,app=app,
        sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in sorted(paths|set(SOURCES))})

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('oracle_log',type=Path);parser.add_argument('app_log',type=Path)
    parser.add_argument('--output',type=Path,default=ROOT/'docs/interface-subd-qualification.json')
    args=parser.parse_args();r=summarize(args.oracle_log.read_text(),args.app_log.read_text())
    args.output.write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    print('SubD interface: {} Rust, {} GUI sequences; {} states per path; no mismatches'.format(r['oracle_replayed'],r['app_replayed'],r['states_per_path']))
if __name__=='__main__':main()

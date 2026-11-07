"""Windowless BlendCrv endpoint trim/join checks and affected regressions."""
import argparse
import gzip
import json
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PROOF = ROOT / 'docs/blend-curve-empty-proof'

def commands():
    suites = ['blend_curve','blend_curve_points','blend_curve_portions',
              'blend_curve_symmetry','blend_curve_edges','blend_curve_surface_high',
              'blend_curve_alt','blend_curve_controls','blend_curve_closed']
    for mode in ['portable','native']:
        features = [] if mode == 'portable' else ['--features','native-smlib']
        yield 'empty-command-'+mode, ['cargo','test','-p','viboceros-command',*features,'--test','blend_curve_empty','--','--nocapture']
        yield 'blend-regressions-'+mode, ['cargo','test','-p','viboceros-command',*features,*[arg for suite in suites for arg in ['--test',suite]],'--','--nocapture']
        yield 'empty-ui-'+mode, ['cargo','test','--bin','viboceros',*features,'app::tests::blend_curve','--','--nocapture']
        yield 'tolerance-library-'+mode, ['cargo','test','-p','viboceros-command',*features,'--lib','model_tolerance::tests']
        yield 'clippy-'+mode, ['cargo','clippy','--workspace','--all-targets',*features,'--','-D','warnings']
    yield 'fmt', ['cargo','fmt','--all','--check']
    yield 'python', ['python3','-m','unittest',*['tools.rhino_oracle.test_'+name for name in ['structural_sdk','client','blend_curve','blend_curve_points','blend_curve_portions','blend_curve_closed','blend_curve_empty']]]
    yield 'subd-command-native', ['cargo','test','-p','viboceros-command','--features','native-smlib','--test','subd_box_native','--test','subd_plane_native','--','--nocapture']
    yield 'subd-ui-native', ['cargo','test','--bin','viboceros','--features','native-smlib','subd_primitives','--','--nocapture']
    yield 'python-subd', ['python3','-m','unittest','tools.rhino_oracle.test_subd_box','tools.rhino_oracle.test_subd_plane']

def refresh_subd():
    from . import report_subd_box, report_subd_plane
    text = '\n'.join(gzip.decompress((PROOF/(name+'.log.gz')).read_bytes()).decode()
                     for name in ['subd-command-native','subd-ui-native'])
    for kind,module in [('box',report_subd_box),('plane',report_subd_plane)]:
        path = ROOT/('docs/subd-'+kind+'-qualification.json')
        before = json.loads(path.read_bytes())
        after = module.summarize(text)
        if {k:v for k,v in before.items() if k != 'sha256'} != {k:v for k,v in after.items() if k != 'sha256'} or set(before['sha256']) != set(after['sha256']):
            raise ValueError('shared SubD behavior or inventory changed')
        after['sha256'] = {p:after['sha256'][p] for p in before['sha256']}
        path.write_text(json.dumps(after,indent=2,allow_nan=False)+'\n')

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--resume', action='store_true')
    args = parser.parse_args()
    PROOF.mkdir(exist_ok=True)
    checks = json.loads((PROOF/'checks.json').read_bytes()) if args.resume else []
    expected = list(commands())
    if [(c['name'],c['command']) for c in checks] != expected[:len(checks)]:
        raise ValueError('saved check commands changed')
    checks = checks[:next((i for i,c in enumerate(checks) if c['status']),len(checks))]
    for name, command in expected[len(checks):]:
        if name == 'python-subd':
            refresh_subd()
        print('Checking '+name, flush=True)
        start = time.monotonic()
        result = subprocess.run(command,cwd=ROOT,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
        (PROOF/(name+'.log.gz')).write_bytes(gzip.compress(result.stdout,mtime=0))
        checks.append(dict(name=name,command=command,status=result.returncode,seconds=time.monotonic()-start))
        (PROOF/'checks.json').write_text(json.dumps(checks,indent=2)+'\n')
        if result.returncode:
            print(result.stdout.decode(errors='replace')[-6000:])
            raise SystemExit(result.returncode)
    print('All endpoint trim/join checks passed', flush=True)

if __name__ == '__main__':
    main()

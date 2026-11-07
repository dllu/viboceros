"""Windowless ArcBlend parallel-radius checks and shared getter/history regressions."""
import argparse
import gzip
import json
import subprocess
import time
from .check_arc_blend_handles import ROOT, commands as prior_commands

PROOF = ROOT / 'docs/arc-blend-radius-proof'

def commands():
    yield 'fmt', ['cargo', 'fmt', '--all', '--', '--check']
    for mode in ['portable', 'native']:
        features = [] if mode == 'portable' else ['--features', 'native-smlib']
        yield 'radius-command-' + mode, ['cargo', 'test', '-p', 'viboceros-command', *features, '--test', 'arc_blend_radius', '--', '--nocapture']
        yield 'radius-ui-' + mode, ['cargo', 'test', '--bin', 'viboceros', *features, 'app::tests::arc_blend_radius', '--', '--nocapture']
    for name, command in prior_commands():
        if name == 'fmt':
            continue
        if name == 'python':
            command = [*command, 'tools.rhino_oracle.test_arc_blend_radius']
        yield name, command

def refresh_subd():
    from . import report_subd_box, report_subd_plane
    text = '\n'.join(gzip.decompress((PROOF / (n + '.log.gz')).read_bytes()).decode() for n in ['commands-native', 'subd-native'])
    for kind, module in [('box', report_subd_box), ('plane', report_subd_plane)]:
        path = ROOT / ('docs/subd-' + kind + '-qualification.json')
        before = json.loads(path.read_bytes())
        after = module.summarize(text)
        if {k: v for k, v in before.items() if k != 'sha256'} != {k: v for k, v in after.items() if k != 'sha256'} or set(before['sha256']) != set(after['sha256']):
            raise ValueError('shared SubD behavior or inventory changed')
        after['sha256'] = {p: after['sha256'][p] for p in before['sha256']}
        path.write_text(json.dumps(after, indent=2, allow_nan=False) + '\n')

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--resume', action='store_true')
    args = parser.parse_args()
    PROOF.mkdir(exist_ok=True)
    checks = json.loads((PROOF / 'checks.json').read_bytes()) if args.resume else []
    expected = list(commands())
    if [(c['name'], c['command']) for c in checks] != expected[:len(checks)]:
        raise ValueError('saved check commands changed')
    checks = checks[:next((i for i, c in enumerate(checks) if c['status']), len(checks))]
    for name, command in expected[len(checks):]:
        if name == 'python':
            refresh_subd()
        print('Checking ' + name, flush=True)
        start = time.monotonic()
        result = subprocess.run(command, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        (PROOF / (name + '.log.gz')).write_bytes(gzip.compress(result.stdout, mtime=0))
        checks.append(dict(name=name, command=command, status=result.returncode, seconds=time.monotonic() - start))
        (PROOF / 'checks.json').write_text(json.dumps(checks, indent=2) + '\n')
        if result.returncode:
            print(result.stdout.decode(errors='replace')[-6000:])
            raise SystemExit(result.returncode)
    print('All ArcBlend checks passed', flush=True)

if __name__ == '__main__':
    main()

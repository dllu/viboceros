"""Run windowless Plane completion and shared-getter regression checks."""
import gzip
import json
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
PROOF = ROOT / 'docs/plane-completion-proof'

def commands():
    yield 'fmt', ['cargo','fmt','--all','--','--check']
    for mode in ['portable','native']:
        features = [] if mode == 'portable' else ['--features','native-smlib']
        yield 'commands-'+mode, ['cargo','test','-p','viboceros-command',*features,
            *sum((['--test',name] for name in ['plane_surface','plane_surface_native','plane_completion','subd_plane_native','subd_box_native']),[]),'--','--nocapture']
        for kind, pattern in [('ui','app::tests::plane_'),('rectangle','rectangle'),('subd','subd_primitives'),('surface-flow','surface_flow')]:
            yield kind+'-'+mode, ['cargo','test','-p','viboceros',*features,'--bin','viboceros',pattern,'--','--nocapture']
    yield 'rectangle-input', ['cargo','test','-p','viboceros-command','--lib','rectangle']
    yield 'clippy-portable', ['cargo','clippy','--workspace','--all-targets','--','-D','warnings']
    yield 'clippy-all-features', ['cargo','clippy','--workspace','--all-targets','--all-features','--','-D','warnings']
    yield 'python', ['python3','-m','unittest',*('tools.rhino_oracle.test_'+name for name in ['plane_completion','plane_surface','rectangle','structural_sdk','client','surface_interpolate_plane','project_plane','subd_plane'])]

def refresh_shared_records():
    # The command and getter logs above actually rerun these qualifications.
    # Refresh rolling source maps only when every behavioral result is equal.
    from . import report_subd_box, report_subd_plane
    text='\n'.join(gzip.decompress((PROOF/(name+'.log.gz')).read_bytes()).decode()
                   for name in ['commands-native','subd-native'])
    for name, module in [('box',report_subd_box),('plane',report_subd_plane)]:
        path=ROOT/('docs/subd-'+name+'-qualification.json')
        before=json.loads(path.read_text());after=module.summarize(text)
        if {k:v for k,v in before.items() if k!='sha256'}!={k:v for k,v in after.items() if k!='sha256'}:
            raise ValueError('shared SubD behavior changed: '+name)
        if set(before['sha256'])!=set(after['sha256']):
            raise ValueError('shared SubD source inventory changed: '+name)
        after['sha256']={key:after['sha256'][key] for key in before['sha256']}
        path.write_text(json.dumps(after,indent=2,allow_nan=False)+'\n')

def main():
    PROOF.mkdir(exist_ok=True)
    checks=[]
    for name, command in commands():
        if name=='python': refresh_shared_records()
        print('Checking '+name,flush=True)
        start=time.monotonic()
        result=subprocess.run(command,cwd=ROOT,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
        (PROOF/(name+'.log.gz')).write_bytes(gzip.compress(result.stdout,mtime=0))
        checks.append(dict(name=name,command=command,status=result.returncode,seconds=time.monotonic()-start))
        (PROOF/'checks.json').write_text(json.dumps(checks,indent=2)+'\n')
        if result.returncode:
            print(result.stdout.decode(errors='replace')[-6000:]);raise SystemExit(result.returncode)
    print('All Plane completion checks passed',flush=True)

if __name__ == '__main__': main()

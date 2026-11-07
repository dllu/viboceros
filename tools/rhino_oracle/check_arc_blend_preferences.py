"""Windowless remembered ArcBlend settings and shared getter/history regressions."""
import argparse
import gzip
import json
import subprocess
import time
from .check_arc_blend_getters import ROOT, commands as prior_commands
PROOF = ROOT / 'docs/arc-blend-preferences-proof'

def commands():
    yield 'fmt', ['cargo','fmt','--all','--','--check']
    for mode in ['portable','native']:
        features=[] if mode=='portable' else ['--features','native-smlib']
        yield 'preferences-command-'+mode,['cargo','test','-p','viboceros-command',*features,'--test','arc_blend_preferences','--','--nocapture']
        yield 'preferences-ui-'+mode,['cargo','test','--bin','viboceros',*features,'app::tests::arc_blend_preferences','--','--nocapture']
    for name,command in prior_commands():
        if name=='fmt':continue
        if name=='python':command=[*command,'tools.rhino_oracle.test_arc_blend_preferences']
        yield name,command

def refresh_subd():
    from . import report_subd_box,report_subd_plane
    text='\n'.join(gzip.decompress((PROOF/(n+'.log.gz')).read_bytes()).decode() for n in ['commands-native','subd-native'])
    for kind,module in [('box',report_subd_box),('plane',report_subd_plane)]:
        path=ROOT/('docs/subd-'+kind+'-qualification.json');before=json.loads(path.read_bytes());after=module.summarize(text)
        if {k:v for k,v in before.items()if k!='sha256'}!={k:v for k,v in after.items()if k!='sha256'}or set(before['sha256'])!=set(after['sha256']):raise ValueError('shared SubD behavior or inventory changed')
        after['sha256']={p:after['sha256'][p]for p in before['sha256']};path.write_text(json.dumps(after,indent=2,allow_nan=False)+'\n')

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--resume',action='store_true');a=p.parse_args()
    PROOF.mkdir(exist_ok=True);checks=json.loads((PROOF/'checks.json').read_bytes())if a.resume else [];expected=list(commands())
    if [(c['name'],c['command'])for c in checks]!=expected[:len(checks)]:raise ValueError('saved check commands changed')
    checks=checks[:next((i for i,c in enumerate(checks)if c['status']),len(checks))]
    for name,command in expected[len(checks):]:
        if name=='python':refresh_subd()
        print('Checking '+name,flush=True);start=time.monotonic();r=subprocess.run(command,cwd=ROOT,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
        (PROOF/(name+'.log.gz')).write_bytes(gzip.compress(r.stdout,mtime=0));checks.append(dict(name=name,command=command,status=r.returncode,seconds=time.monotonic()-start));(PROOF/'checks.json').write_text(json.dumps(checks,indent=2)+'\n')
        if r.returncode:print(r.stdout.decode(errors='replace')[-6000:]);raise SystemExit(r.returncode)
    print('All ArcBlend checks passed',flush=True)
if __name__=='__main__':main()

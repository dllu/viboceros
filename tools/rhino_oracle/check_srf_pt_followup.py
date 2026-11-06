"""Qualify remote corner admission without replacing the historical proof."""
import gzip
import json
import subprocess
import time
from .check_srf_pt import ROOT,commands as original_commands

PROOF=ROOT/'docs/srf-pt-followup-proof'

def commands():
    # The fix changes triangle admission, not the shared surface constructor.
    retained={'fmt','creation','commands-portable','commands-native','ui-portable','ui-native',
        'subd-native','clippy-portable','clippy-all-features','python'}
    for name,command in original_commands():
        if name not in retained:continue
        if name=='python':command=[*command,'tools.rhino_oracle.test_srf_pt_followup']
        yield name,command

def main():
    PROOF.mkdir(exist_ok=True);checks=[]
    for name,command in commands():
        if name=='python':
            from . import report_subd_box,report_subd_plane
            text='\n'.join(gzip.decompress((PROOF/(n+'.log.gz')).read_bytes()).decode() for n in ['commands-native','subd-native'])
            for kind,module in [('box',report_subd_box),('plane',report_subd_plane)]:
                path=ROOT/('docs/subd-'+kind+'-qualification.json');before=json.loads(path.read_text());after=module.summarize(text)
                if {k:v for k,v in before.items() if k!='sha256'}!={k:v for k,v in after.items() if k!='sha256'}:raise ValueError('shared SubD behavior changed')
                if set(before['sha256'])!=set(after['sha256']):raise ValueError('shared source inventory changed')
                after['sha256']={p:after['sha256'][p] for p in before['sha256']}
                path.write_text(json.dumps(after,indent=2,allow_nan=False)+'\n')
        print('Checking '+name,flush=True);start=time.monotonic()
        result=subprocess.run(command,cwd=ROOT,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
        (PROOF/(name+'.log.gz')).write_bytes(gzip.compress(result.stdout,mtime=0))
        checks.append(dict(name=name,command=command,status=result.returncode,seconds=time.monotonic()-start))
        (PROOF/'checks.json').write_text(json.dumps(checks,indent=2)+'\n')
        if result.returncode:
            print(result.stdout.decode(errors='replace')[-6000:]);raise SystemExit(result.returncode)
    print('All corner-surface followup checks passed',flush=True)

if __name__=='__main__':main()

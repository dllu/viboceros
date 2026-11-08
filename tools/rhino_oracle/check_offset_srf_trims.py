"""Windowless trimmed-offset qualification and affected regression checks."""
from .check_offset_srf_tolerance import ROOT, commands as tolerance_commands
PROOF = ROOT/'docs/offset-srf-trims-proof'

def commands():
    yield 'geometry', ['cargo','test','-p','viboceros-geometry','--lib']
    for name, command in tolerance_commands():
        if name == 'command-native':
            yield 'native-sheet-export',['cargo','test','-p','viboceros-smlib','--features','native','--test','sheets']
            yield 'trim-curvature-guard',['cargo','test','-p','viboceros-command','--features','native-smlib','--test','offset_surface_tolerance','trimmed_curvature_breaks','--','--nocapture']
            yield name,[*command,'--skip','trimmed_offsets']
            yield 'trim-command-native',[*command[:-2],'trimmed_offsets','--','--nocapture']
            continue
        if name == 'python':
            command = [*command, 'tools.rhino_oracle.test_offset_srf_trims']
        yield name, command

def main():
    import argparse
    import gzip
    import json
    import subprocess
    import time
    from . import check_blend_curve_empty as runner
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--resume',action='store_true'); args=parser.parse_args()
    PROOF.mkdir(exist_ok=True)
    expected=list(commands()); inventory=dict(expected)
    saved=json.loads((PROOF/'checks.json').read_bytes()) if args.resume else []
    done={c['name']:c for c in saved}
    if len(done)!=len(saved) or any(c['name'] not in inventory or c['command']!=inventory[c['name']] for c in saved): raise ValueError('saved check commands changed')
    for name,command in expected:
        if name in done and done[name]['status']==0: continue
        if name=='python-subd':
            runner.PROOF=PROOF; runner.refresh_subd()
        print('Checking '+name,flush=True); start=time.monotonic()
        result=subprocess.run(command,cwd=ROOT,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
        (PROOF/(name+'.log.gz')).write_bytes(gzip.compress(result.stdout,mtime=0))
        done[name]=dict(name=name,command=command,status=result.returncode,seconds=time.monotonic()-start)
        (PROOF/'checks.json').write_text(json.dumps([done[n] for n,_ in expected if n in done],indent=2)+'\n')
        if result.returncode:
            print(result.stdout.decode(errors='replace')[-6000:]); raise SystemExit(result.returncode)
    print('All trimmed-offset checks passed',flush=True)

if __name__ == '__main__': main()

"""Windowless loose offsets, shared trim edits and affected command regressions."""
from .check_blend_curve_empty import ROOT
PROOF=ROOT/'docs/offset-srf-loose-proof'

def commands():
    for mode in ['portable','native']:
        features=[]if mode=='portable'else['--features','native-smlib']
        yield 'command-'+mode,['cargo','test','-p','viboceros-command',*features,'--test','offset_surface_loose','--','--nocapture']
        yield 'ui-'+mode,['cargo','test','--bin','viboceros',*features,'app::tests::offset_surface_loose','--','--nocapture']
        yield 'offset-library-'+mode,['cargo','test','-p','viboceros-command',*features,'--lib','offset_surface::','--','--nocapture']
        yield 'offset-command-'+mode,['cargo','test','-p','viboceros-command',*features,'--test','offset_surface_getter','--','--nocapture']
        yield 'offset-ui-'+mode,['cargo','test','--bin','viboceros',*features,'app::tests::offset_surface::','--','--nocapture']
        yield 'shell-command-'+mode,['cargo','test','-p','viboceros-command',*features,'--test','shell_native','--','--nocapture']
        yield 'shell-ui-'+mode,['cargo','test','--bin','viboceros',*features,'app::tests::shell','--','--nocapture']
        yield 'clippy-'+mode,['cargo','clippy','--workspace','--all-targets',*features,'--','-D','warnings']
    yield 'geometry',['cargo','test','-p','viboceros-geometry','--lib']
    yield 'controls',['cargo','test','-p','viboceros-geometry','--test','loose_offset_native','--','--nocapture']
    yield 'fmt',['cargo','fmt','--all','--check']
    yield 'python',['python3','-m','unittest',*['tools.rhino_oracle.test_'+n for n in [
        'offset_srf_loose','offset_surface_getter','shell_native','capture_family','structural_sdk','client']]]
    yield 'subd-command-native',['cargo','test','-p','viboceros-command','--features','native-smlib','--test','subd_box_native','--test','subd_plane_native','--','--nocapture']
    yield 'subd-ui-native',['cargo','test','--bin','viboceros','--features','native-smlib','subd_primitives','--','--nocapture']
    yield 'python-subd',['python3','-m','unittest','tools.rhino_oracle.test_subd_box','tools.rhino_oracle.test_subd_plane']

def main():
    from . import check_blend_curve_empty as runner
    import hashlib,json
    from types import SimpleNamespace
    run=runner.subprocess.run
    def checked_run(command,**kwargs):
        # A full geometry run can finish while the independent command/UI
        # checks run. Reuse it only with identical geometry sources and log.
        path=PROOF/'geometry-prechecked.json'
        if command==['cargo','test','-p','viboceros-geometry','--lib']and path.exists():
            receipt=json.loads(path.read_bytes())
            digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
            if receipt['command']!=command or receipt['status']!=0 or any(digest(ROOT/p)!=v for p,v in receipt['sources'].items()):raise ValueError('stale geometry receipt')
            log=PROOF/receipt['log']
            if digest(log)!=receipt['log_sha256']:raise ValueError('geometry log hash')
            print('Reusing completed full geometry suite with verified sources',flush=True)
            import gzip
            return SimpleNamespace(returncode=0,stdout=gzip.decompress(log.read_bytes()))
        return run(command,**kwargs)
    runner.subprocess.run=checked_run
    runner.PROOF=PROOF;runner.commands=commands
    try:runner.main()
    finally:runner.subprocess.run=run

if __name__=='__main__':main()

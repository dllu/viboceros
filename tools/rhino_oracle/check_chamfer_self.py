"""Windowless same-parent Chamfer and shared Fillet admission qualification."""
from .check_blend_curve_empty import ROOT
PROOF=ROOT/'docs/chamfer-self-proof'
def commands():
    for mode in ['portable','native']:
        features=[] if mode=='portable' else ['--features','native-smlib']
        yield 'self-command-'+mode,['cargo','test','-p','viboceros-command',*features,'--test','chamfer_self','--','--nocapture']
        yield 'self-ui-'+mode,['cargo','test','--bin','viboceros',*features,'app::tests::chamfer_self','--','--nocapture']
        suites=['chamfer_getter','fillet_getter','fillet_closed','fillet_self']
        yield 'command-regressions-'+mode,['cargo','test','-p','viboceros-command',*features,*[arg for suite in suites for arg in ['--test',suite]],'--','--nocapture']
        yield 'chamfer-ui-'+mode,['cargo','test','--bin','viboceros',*features,'app::tests::curve_chamfer','--','--nocapture']
        yield 'fillet-ui-'+mode,['cargo','test','--bin','viboceros',*features,'fillet','--','--nocapture']
        for command in ['chamfer','fillet']:
            yield command+'-library-'+mode,['cargo','test','-p','viboceros-command',*features,'--lib',command+'::tests','--','--nocapture']
        yield 'clippy-'+mode,['cargo','clippy','--workspace','--all-targets',*features,'--','-D','warnings']
    yield 'fmt',['cargo','fmt','--all','--check']
    yield 'python',['python3','-m','unittest','tools.rhino_oracle.test_structural_sdk','tools.rhino_oracle.test_client','tools.rhino_oracle.test_chamfer_getter','tools.rhino_oracle.test_chamfer_self']
    yield 'subd-command-native',['cargo','test','-p','viboceros-command','--features','native-smlib','--test','subd_box_native','--test','subd_plane_native','--','--nocapture']
    yield 'subd-ui-native',['cargo','test','--bin','viboceros','--features','native-smlib','subd_primitives','--','--nocapture']
    yield 'python-subd',['python3','-m','unittest','tools.rhino_oracle.test_subd_box','tools.rhino_oracle.test_subd_plane']
def main():
    from . import check_blend_curve_empty as runner
    runner.PROOF=PROOF;runner.commands=commands;runner.main()
if __name__=='__main__':main()

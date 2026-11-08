"""Windowless offset getters, kernel geometry, UI picks and shared regression gates."""
from .check_blend_curve_empty import ROOT
PROOF=ROOT/'docs/offset-surface-getter-proof'

def commands():
    for mode in ['portable','native']:
        features=[] if mode=='portable' else ['--features','native-smlib']
        yield 'command-'+mode,['cargo','test','-p','viboceros-command',*features,'--test','offset_surface_getter','--','--nocapture']
        yield 'ui-'+mode,['cargo','test','--bin','viboceros',*features,'app::tests::offset_surface','--','--nocapture']
        yield 'library-'+mode,['cargo','test','-p','viboceros-command',*features,'--lib','offset_surface','--','--nocapture']
        yield 'direction-ui-'+mode,['cargo','test','--bin','viboceros',*features,'app::tests::direction','--','--nocapture']
        for suite in ['curve_fillet','curve_chamfer']:
            yield suite+'-ui-'+mode,['cargo','test','--bin','viboceros',*features,'app::tests::'+suite,'--','--nocapture']
        yield 'clippy-'+mode,['cargo','clippy','--workspace','--all-targets',*features,'--','-D','warnings']
    yield 'native-producers',['cargo','test','-p','viboceros-smlib','--features','native','--test','shell','--test','sheets','--','--nocapture']
    yield 'fmt',['cargo','fmt','--all','--check']
    yield 'python',['python3','-m','unittest',*['tools.rhino_oracle.test_'+name for name in ['structural_sdk','client','offset_native','surface_offset_command','chamfer_getter','chamfer_open','offset_surface_getter']]]
    yield 'subd-command-native',['cargo','test','-p','viboceros-command','--features','native-smlib','--test','subd_box_native','--test','subd_plane_native','--','--nocapture']
    yield 'subd-ui-native',['cargo','test','--bin','viboceros','--features','native-smlib','subd_primitives','--','--nocapture']
    yield 'python-subd',['python3','-m','unittest','tools.rhino_oracle.test_subd_box','tools.rhino_oracle.test_subd_plane']

def main():
    from . import check_blend_curve_empty as runner
    runner.PROOF=PROOF;runner.commands=commands;runner.main()

if __name__=='__main__':main()

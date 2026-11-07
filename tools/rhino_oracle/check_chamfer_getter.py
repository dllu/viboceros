"""Windowless Chamfer qualification and regressions for shared curve admission."""
from .check_blend_curve_empty import ROOT
PROOF = ROOT / 'docs/chamfer-getter-proof'

def commands():
    yield 'geometry', ['cargo','test','-p','viboceros-geometry','--lib','curve_chamfer_pair','--','--nocapture']
    yield 'chord-geometry', ['cargo','test','-p','viboceros-geometry','--lib','curve::chord_division','--','--nocapture']
    for mode in ['portable','native']:
        features = [] if mode == 'portable' else ['--features','native-smlib']
        yield 'command-'+mode, ['cargo','test','-p','viboceros-command',*features,'--test','chamfer_getter','--','--nocapture']
        yield 'library-'+mode, ['cargo','test','-p','viboceros-command',*features,'--lib','chamfer::tests','--','--nocapture']
        yield 'ui-'+mode, ['cargo','test','--bin','viboceros',*features,'app::tests::curve_chamfer','--','--nocapture']
        yield 'fillet-command-'+mode, ['cargo','test','-p','viboceros-command',*features,'--test','fillet_getter','--test','fillet_closed','--test','fillet_self','--','--nocapture']
        yield 'fillet-ui-'+mode, ['cargo','test','--bin','viboceros',*features,'fillet','--','--nocapture']
        yield 'clippy-'+mode, ['cargo','clippy','--workspace','--all-targets',*features,'--','-D','warnings']
    yield 'fmt', ['cargo','fmt','--all','--check']
    yield 'python', ['python3','-m','unittest','tools.rhino_oracle.test_structural_sdk','tools.rhino_oracle.test_client','tools.rhino_oracle.test_chamfer_getter']
    yield 'subd-command-native', ['cargo','test','-p','viboceros-command','--features','native-smlib','--test','subd_box_native','--test','subd_plane_native','--','--nocapture']
    yield 'subd-ui-native', ['cargo','test','--bin','viboceros','--features','native-smlib','subd_primitives','--','--nocapture']
    yield 'python-subd', ['python3','-m','unittest','tools.rhino_oracle.test_subd_box','tools.rhino_oracle.test_subd_plane']
    yield 'native-connections', ['cargo','test','-p','viboceros-smlib','--features','native','--test','curve_connections','--','--nocapture']
    yield 'connection-geometry', ['cargo','test','-p','viboceros-geometry','--lib','curve_connect_pair','--','--nocapture']

def main():
    from . import check_blend_curve_empty as runner
    runner.PROOF=PROOF;runner.commands=commands;runner.main()
if __name__=='__main__': main()

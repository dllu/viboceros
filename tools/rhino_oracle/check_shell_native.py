"""Windowless Shell, trim-region, native transfer and shared-registry checks."""
from .check_blend_curve_empty import ROOT
PROOF = ROOT/'docs/shell-native-proof'


def commands():
    for mode in ['portable', 'native']:
        features = [] if mode == 'portable' else ['--features', 'native-smlib']
        yield 'command-'+mode, ['cargo','test','-p','viboceros-command',*features,'--test','shell_native','--','--nocapture']
        yield 'ui-'+mode, ['cargo','test','--bin','viboceros',*features,'app::tests::shell','--','--nocapture']
        yield 'library-'+mode, ['cargo','test','-p','viboceros-command',*features,'--lib','shell::','--','--nocapture']
        yield 'oracle-'+mode, ['cargo','test','-p','viboceros-oracle',*features,'--lib','shell::','--','--nocapture']
        yield 'offset-command-'+mode, ['cargo','test','-p','viboceros-command',*features,'--test','offset_surface_getter','--','--nocapture']
        yield 'offset-ui-'+mode, ['cargo','test','--bin','viboceros',*features,'app::tests::offset_surface','--','--nocapture']
        yield 'clippy-'+mode, ['cargo','clippy','--workspace','--all-targets',*features,'--','-D','warnings']
    yield 'geometry', ['cargo','test','-p','viboceros-geometry','--lib','brep::']
    yield 'native-producers', ['cargo','test','-p','viboceros-smlib','--features','native','--test','shell','--test','brep_transfer','--test','brep_import','--','--nocapture']
    yield 'native-joints', ['cargo','test','-p','viboceros-smlib','--features','native','--lib','native::brep::joints','--','--nocapture']
    yield 'fmt', ['cargo','fmt','--all','--check']
    yield 'python', ['python3','-m','unittest',*['tools.rhino_oracle.test_'+name for name in [
        'capture_family','shell','shell_command','shell_native','structural_sdk','client','offset_surface_getter']]]
    yield 'subd-command-native', ['cargo','test','-p','viboceros-command','--features','native-smlib','--test','subd_box_native','--test','subd_plane_native','--','--nocapture']
    yield 'subd-ui-native', ['cargo','test','--bin','viboceros','--features','native-smlib','subd_primitives','--','--nocapture']
    yield 'python-subd', ['python3','-m','unittest','tools.rhino_oracle.test_subd_box','tools.rhino_oracle.test_subd_plane']


def main():
    from . import check_blend_curve_empty as runner
    runner.PROOF = PROOF; runner.commands = commands; runner.main()


if __name__ == '__main__':
    main()

"""Windowless tolerance, offset and adjacent native regression checks."""
from .check_blend_curve_empty import ROOT
PROOF=ROOT/'docs/offset-srf-tolerance-proof'

def commands():
    for mode in ['portable','native']:
        features=[]if mode=='portable'else['--features','native-smlib']
        yield 'command-'+mode,['cargo','test','-p','viboceros-command',*features,'--test','offset_surface_tolerance','--','--nocapture']
        yield 'ui-'+mode,['cargo','test','--bin','viboceros',*features,'app::tests::offset_surface','--','--nocapture']
        yield 'offset-library-'+mode,['cargo','test','-p','viboceros-command',*features,'--lib','offset_surface::','--','--nocapture']
        yield 'offset-regressions-'+mode,['cargo','test','-p','viboceros-command',*features,'--test','offset_surface_getter','--test','offset_surface_loose','--','--nocapture']
        yield 'shell-command-'+mode,['cargo','test','-p','viboceros-command',*features,'--test','shell_native','--','--nocapture']
        yield 'shell-ui-'+mode,['cargo','test','--bin','viboceros',*features,'app::tests::shell','--','--nocapture']
        yield 'clippy-'+mode,['cargo','clippy','--workspace','--all-targets',*features,'--','-D','warnings']
    yield 'fmt',['cargo','fmt','--all','--check']
    yield 'python',['python3','-m','unittest',*['tools.rhino_oracle.test_'+name for name in [
        'offset_srf_tolerance','offset_srf_loose','offset_surface_getter','shell_native','capture_family','structural_sdk','client']]]
    yield 'subd-command-native',['cargo','test','-p','viboceros-command','--features','native-smlib','--test','subd_box_native','--test','subd_plane_native','--','--nocapture']
    yield 'subd-ui-native',['cargo','test','--bin','viboceros','--features','native-smlib','subd_primitives','--','--nocapture']
    yield 'python-subd',['python3','-m','unittest','tools.rhino_oracle.test_subd_box','tools.rhino_oracle.test_subd_plane']

if __name__=='__main__':
    from . import check_blend_curve_empty as runner
    runner.PROOF=PROOF;runner.commands=commands;runner.main()

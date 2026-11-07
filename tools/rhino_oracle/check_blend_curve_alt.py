"""Alt surface angles and the command/editor regressions they can affect."""
from .check_blend_curve_surface_high import ROOT,commands as prior_commands
PROOF=ROOT/'docs/blend-curve-alt-proof'
def commands():
    for mode in ['portable','native']:
        features=[]if mode=='portable'else['--features','native-smlib']
        yield 'alt-command-'+mode,['cargo','test','-p','viboceros-command',*features,'--test','blend_curve_alt','--','--nocapture']
        yield 'alt-ui-'+mode,['cargo','test','--bin','viboceros',*features,'app::tests::blend_curve_alt','--','--nocapture']
    prefixes=('surface-high-','edge-','symmetry-','portion-','point-command-','point-ui-','mixed-orders-','blend-command-','blend-ui-','arc-ui-','normal-ui-','pick-viewport-','primitive-input-','clippy-')
    names={'fourth-derivatives','endpoint-jets','native-blend-interpolation','fmt','registry','history','commands-native','subd-native','python'}
    for name,command in prior_commands():
        if name.startswith(prefixes)or name in names:
            if name=='python':command=[*command,'tools.rhino_oracle.test_blend_curve_alt']
            yield name,command
def main():
    from . import check_blend_curve_points as runner
    runner.PROOF=PROOF;runner.commands=commands;runner.main()
if __name__=='__main__':main()

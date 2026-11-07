"""Third surface jets, higher-order surface blends and inherited regressions."""
from .check_blend_curve_edges import ROOT,commands as prior_commands
PROOF=ROOT/'docs/blend-curve-surface-high-proof'
def commands():
    yield 'surface-third-kernel',['cargo','test','-p','viboceros-geometry','--lib','--','--nocapture']
    yield 'surface-third-native',['cargo','test','-p','viboceros-geometry','--test','surface_third_native','--','--nocapture']
    for mode in ['portable','native']:
        features=[]if mode=='portable'else['--features','native-smlib']
        yield 'surface-high-command-'+mode,['cargo','test','-p','viboceros-command',*features,'--test','blend_curve_surface_high','--','--nocapture']
        yield 'surface-high-ui-'+mode,['cargo','test','--bin','viboceros',*features,'app::tests::blend_curve_surface_high','--','--nocapture']
    for name,command in prior_commands():
        if name=='python':command=[*command,'tools.rhino_oracle.test_blend_curve_surface_high']
        yield name,command
def main():
    from . import check_blend_curve_points as runner
    runner.PROOF=PROOF;runner.commands=commands;runner.main()
if __name__=='__main__':main()

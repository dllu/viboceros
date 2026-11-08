"""Windowless Chamfer contacts, consumed sources and shared Fillet regressions."""
from .check_blend_curve_empty import ROOT
PROOF = ROOT / 'docs/chamfer-open-proof'


def commands():
    for mode in ['portable', 'native']:
        features = [] if mode == 'portable' else ['--features', 'native-smlib']
        yield 'open-command-'+mode, ['cargo','test','-p','viboceros-command',*features,'--test','chamfer_open','--','--nocapture']
        yield 'open-ui-'+mode, ['cargo','test','--bin','viboceros',*features,'app::tests::chamfer_open','--','--nocapture']
        suites = ['fillet_open_zero','fillet_zero','fillet_continuation','fillet_getter','fillet_closed','fillet_self','chamfer_getter','chamfer_closed','chamfer_self','chamfer_extension','chamfer_branch']
        yield 'command-regressions-'+mode, ['cargo','test','-p','viboceros-command',*features,*[a for s in suites for a in ['--test',s]],'--','--nocapture']
        for label, suite in [('open-zero-ui','app::tests::fillet_open_zero'),('zero-ui','app::tests::fillet_zero'),('fillet-ui','app::tests::curve_fillet'),('closed-ui','app::tests::fillet_closed'),('self-ui','app::tests::fillet_self'),('continuation-ui','app::tests::fillet_continuation'),('chamfer-ui','chamfer')]:
            skip = ['--skip','app::tests::chamfer_open'] if label == 'chamfer-ui' else []
            yield label+'-'+mode, ['cargo','test','--bin','viboceros',*features,suite,'--','--nocapture',*skip]
        yield 'library-'+mode, ['cargo','test','-p','viboceros-command',*features,'--lib','fillet::','--','--nocapture']
        yield 'chamfer-library-'+mode, ['cargo','test','-p','viboceros-command',*features,'--lib','chamfer::','--','--nocapture']
        yield 'clippy-'+mode, ['cargo','clippy','--workspace','--all-targets',*features,'--','-D','warnings']
    yield 'tangent-geometry', ['cargo','test','-p','viboceros-geometry','--lib','circle_tangent','--','--nocapture']
    yield 'native-producers', ['cargo','test','-p','viboceros-smlib','--features','native','--test','curve_extension','--test','boundary_extension','--test','curve_intersections','--','--nocapture']
    yield 'fmt', ['cargo','fmt','--all','--check']
    yield 'python', ['python3','-m','unittest',*['tools.rhino_oracle.test_'+s for s in ['structural_sdk','client','fillet_getter','fillet_closed','fillet_self','chamfer_getter','chamfer_closed','chamfer_self','chamfer_extension','chamfer_branch','fillet_continuation','fillet_zero','fillet_open_zero','chamfer_open']]]
    yield 'subd-command-native', ['cargo','test','-p','viboceros-command','--features','native-smlib','--test','subd_box_native','--test','subd_plane_native','--','--nocapture']
    yield 'subd-ui-native', ['cargo','test','--bin','viboceros','--features','native-smlib','subd_primitives','--','--nocapture']
    yield 'python-subd', ['python3','-m','unittest','tools.rhino_oracle.test_subd_box','tools.rhino_oracle.test_subd_plane']


def main():
    from . import check_blend_curve_empty as runner
    runner.PROOF = PROOF
    runner.commands = commands
    runner.main()


if __name__ == '__main__': main()

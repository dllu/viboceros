"""Windowless same-parent Fillet checks and distinct-source regressions."""
from .check_blend_curve_empty import ROOT
PROOF = ROOT / 'docs/fillet-self-proof'


def commands():
    for mode in ['portable', 'native']:
        features = [] if mode == 'portable' else ['--features', 'native-smlib']
        yield 'self-command-' + mode, ['cargo', 'test', '-p', 'viboceros-command', *features, '--test', 'fillet_self', '--', '--nocapture']
        yield 'command-regressions-' + mode, ['cargo', 'test', '-p', 'viboceros-command', *features, '--test', 'fillet_getter', '--test', 'fillet_closed', '--', '--nocapture']
        for name, suite in [('self-ui', 'fillet_self'), ('open-ui', 'curve_fillet'), ('closed-ui', 'fillet_closed')]:
            yield name + '-' + mode, ['cargo', 'test', '--bin', 'viboceros', *features, 'app::tests::' + suite, '--', '--nocapture']
        yield 'library-' + mode, ['cargo', 'test', '-p', 'viboceros-command', *features, '--lib', 'fillet::tests', '--', '--nocapture']
        yield 'clippy-' + mode, ['cargo', 'clippy', '--workspace', '--all-targets', *features, '--', '-D', 'warnings']
    yield 'geometry-decompose', ['cargo', 'test', '-p', 'viboceros-geometry', '--lib', 'nurbs::decompose', '--', '--nocapture']
    yield 'fmt', ['cargo', 'fmt', '--all', '--check']
    yield 'python', ['python3', '-m', 'unittest', 'tools.rhino_oracle.test_structural_sdk', 'tools.rhino_oracle.test_client', 'tools.rhino_oracle.test_fillet_getter', 'tools.rhino_oracle.test_fillet_closed', 'tools.rhino_oracle.test_fillet_self']
    yield 'subd-command-native', ['cargo', 'test', '-p', 'viboceros-command', '--features', 'native-smlib', '--test', 'subd_box_native', '--test', 'subd_plane_native', '--', '--nocapture']
    yield 'subd-ui-native', ['cargo', 'test', '--bin', 'viboceros', '--features', 'native-smlib', 'subd_primitives', '--', '--nocapture']
    yield 'python-subd', ['python3', '-m', 'unittest', 'tools.rhino_oracle.test_subd_box', 'tools.rhino_oracle.test_subd_plane']


def main():
    from . import check_blend_curve_empty as runner
    runner.PROOF = PROOF; runner.commands = commands; runner.main()

if __name__ == '__main__': main()

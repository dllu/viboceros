"""Windowless closed-source Fillet qualification and open-source regressions."""
from .check_blend_curve_empty import ROOT
PROOF = ROOT / 'docs/fillet-closed-proof'


def commands():
    for mode in ['portable', 'native']:
        features = [] if mode == 'portable' else ['--features', 'native-smlib']
        for name, suite in [('closed-command', 'fillet_closed'), ('open-command', 'fillet_getter')]:
            yield name + '-' + mode, ['cargo', 'test', '-p', 'viboceros-command', *features, '--test', suite, '--', '--nocapture']
        for name, suite in [('closed-ui', 'fillet_closed'), ('open-ui', 'curve_fillet')]:
            yield name + '-' + mode, ['cargo', 'test', '--bin', 'viboceros', *features, 'app::tests::' + suite, '--', '--nocapture']
        yield 'library-' + mode, ['cargo', 'test', '-p', 'viboceros-command', *features, '--lib', 'fillet::tests', '--', '--nocapture']
        yield 'clippy-' + mode, ['cargo', 'clippy', '--workspace', '--all-targets', *features, '--', '-D', 'warnings']
    yield 'tangent-geometry', ['cargo', 'test', '-p', 'viboceros-geometry', 'circle_tangent', '--', '--nocapture']
    yield 'fmt', ['cargo', 'fmt', '--all', '--check']
    yield 'python', ['python3', '-m', 'unittest', 'tools.rhino_oracle.test_structural_sdk', 'tools.rhino_oracle.test_client', 'tools.rhino_oracle.test_fillet_getter', 'tools.rhino_oracle.test_fillet_closed']
    yield 'subd-command-native', ['cargo', 'test', '-p', 'viboceros-command', '--features', 'native-smlib', '--test', 'subd_box_native', '--test', 'subd_plane_native', '--', '--nocapture']
    yield 'subd-ui-native', ['cargo', 'test', '--bin', 'viboceros', '--features', 'native-smlib', 'subd_primitives', '--', '--nocapture']
    yield 'python-subd', ['python3', '-m', 'unittest', 'tools.rhino_oracle.test_subd_box', 'tools.rhino_oracle.test_subd_plane']


def main():
    from . import check_blend_curve_empty as runner
    runner.PROOF = PROOF; runner.commands = commands; runner.main()

if __name__ == '__main__': main()

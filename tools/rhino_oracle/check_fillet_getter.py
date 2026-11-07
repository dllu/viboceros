"""Windowless Fillet command, UI, preview and affected shared-protocol checks."""
from .check_blend_curve_empty import ROOT
PROOF = ROOT / 'docs/fillet-getter-proof'

def commands():
    for mode in ['portable', 'native']:
        features = [] if mode == 'portable' else ['--features', 'native-smlib']
        if mode == 'native':
            yield 'native-contacts', ['cargo', 'test', '-p', 'viboceros-smlib', '--features', 'native', '--test', 'fillet_contacts', '--', '--nocapture']
        yield 'command-' + mode, ['cargo', 'test', '-p', 'viboceros-command', *features, '--test', 'fillet_getter', '--', '--nocapture']
        yield 'library-' + mode, ['cargo', 'test', '-p', 'viboceros-command', *features, '--lib', 'fillet::tests', '--', '--nocapture']
        yield 'ui-' + mode, ['cargo', 'test', '--bin', 'viboceros', *features, 'app::tests::curve_fillet', '--', '--nocapture']
        yield 'blend-ui-' + mode, ['cargo', 'test', '--bin', 'viboceros', *features, 'app::tests::blend_curve', '--', '--nocapture']
        yield 'clippy-' + mode, ['cargo', 'clippy', '--workspace', '--all-targets', *features, '--', '-D', 'warnings']
    yield 'fmt', ['cargo', 'fmt', '--all', '--check']
    yield 'python', ['python3', '-m', 'unittest', 'tools.rhino_oracle.test_structural_sdk', 'tools.rhino_oracle.test_client', 'tools.rhino_oracle.test_fillet_getter']
    yield 'subd-command-native', ['cargo', 'test', '-p', 'viboceros-command', '--features', 'native-smlib', '--test', 'subd_box_native', '--test', 'subd_plane_native', '--', '--nocapture']
    yield 'subd-ui-native', ['cargo', 'test', '--bin', 'viboceros', '--features', 'native-smlib', 'subd_primitives', '--', '--nocapture']
    yield 'python-subd', ['python3', '-m', 'unittest', 'tools.rhino_oracle.test_subd_box', 'tools.rhino_oracle.test_subd_plane']

def main():
    from . import check_blend_curve_empty as runner
    runner.PROOF = PROOF; runner.commands = commands; runner.main()

if __name__ == '__main__': main()

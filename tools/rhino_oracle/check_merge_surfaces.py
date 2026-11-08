"""Windowless surface-support merging and shared adapter regressions."""
from .check_offset_srf_trims import ROOT
PROOF = ROOT / 'docs/merge-surfaces-proof'


def commands():
    yield 'kernel-native', ['cargo', 'test', '-p', 'viboceros-smlib', '--features', 'native',
        '--test', 'surface_merge', '--test', 'surface_offset', '--test', 'sheets', '--', '--nocapture']
    for mode in ['portable', 'native']:
        features = [] if mode == 'portable' else ['--features', 'native-smlib']
        yield 'clippy-' + mode, ['cargo', 'clippy', '--workspace', '--all-targets', *features, '--', '-D', 'warnings']
    yield 'fmt', ['cargo', 'fmt', '--all', '--check']
    yield 'subd-command-native', ['cargo', 'test', '-p', 'viboceros-command', '--features', 'native-smlib',
        '--test', 'subd_box_native', '--test', 'subd_plane_native', '--', '--nocapture']
    yield 'subd-ui-native', ['cargo', 'test', '--bin', 'viboceros', '--features', 'native-smlib', 'subd_primitives', '--', '--nocapture']
    yield 'python-subd', ['python3', '-m', 'unittest', 'tools.rhino_oracle.test_subd_box', 'tools.rhino_oracle.test_subd_plane']
    yield 'python', ['python3', '-m', 'unittest', *['tools.rhino_oracle.test_' + name for name in [
        'merge_surfaces', 'client', 'capture_family', 'structural_sdk']]]


if __name__ == '__main__':
    from . import check_offset_srf_trims as runner
    runner.PROOF = PROOF
    runner.commands = commands
    runner.main()

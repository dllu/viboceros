"""Freeze public SDK surface insets on planar, curved and joined faces."""
from .capture_family import capture
from .inset_surface_probe import cases
import argparse


def request():
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-6),
                operations=cases())


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--command', action='store_true')
    parser.add_argument('--ready', action='store_true', help='command cases with the Loose=Off spelling')
    args = parser.parse_args()
    query = request()
    sources = ['generate_inset_surface.py', 'inset_surface_probe.py', 'client.py',
               'rhino_worker.py', 'structural_sdk.py', 'merge_edges_probe.py']
    if args.command or args.ready:
        from .inset_surface_command import cases as command_cases
        query['operations'] = command_cases(args.ready)
        sources += ['inset_surface_command.py', 'join_probe.py']
    capture('inset_surface_ready' if args.ready else 'inset_surface_command' if args.command else 'inset_surface_sdk', query,
            sources, 'VibocerosOracleInsetSurfaceReady20261102' if args.ready else 'VibocerosOracleInsetSurfaceCommand20261102' if args.command else
            'VibocerosOracleInsetSurface20261102', timeout=1200)

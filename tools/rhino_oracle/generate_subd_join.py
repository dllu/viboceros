"""Freeze native SubD joins, including creases, gaps and competing boundaries."""
from .capture_family import capture
from .subd_join_cases import SCENES,validate
import argparse


def request(commands=False,history=False):
    if commands:
        from .subd_join_command_probe import SCENES as COMMAND_SCENES,validate as check
        operations=[dict(op='subd_join_history' if history else 'subd_join_command',id='-'.join((name,command,'pre' if preselect else 'post','crease' if crease else 'smooth')),
            scene=name,command=command,preselect=preselect,crease=crease)
            for name in COMMAND_SCENES for command in ('Join','JoinCopy') for preselect in (False,True)
            for crease in ((False,True) if name=='adjacent' else (False,))]
        for op in operations:check(op)
        return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-8,relative=1e-12,angular=1e-8),operations=operations)
    operations=[dict(op='subd_join_sdk',id=name+('-crease' if crease else '-smooth'),scene=name,crease=crease)
                for name in SCENES for crease in (False,True)]
    for op in operations:validate(op)
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-8,relative=1e-12,angular=1e-8),operations=operations)


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--commands',action='store_true');parser.add_argument('--history',action='store_true');args=parser.parse_args()
    args.commands=args.commands or args.history
    sources=['generate_subd_join.py','subd_join_cases.py','subd_join_probe.py',
        'subd_box_probe.py','client.py','rhino_worker.py','structural_sdk.py','merge_edges_probe.py']
    if args.commands:sources+=['subd_join_command_probe.py','join_probe.py']
    capture('subd_join_history' if args.history else 'subd_join_command' if args.commands else 'subd_join_sdk',request(args.commands,args.history),sources,
        'VibocerosOracleSubDJoinHistory20261102' if args.history else 'VibocerosOracleSubDJoinCommand20261102' if args.commands else 'VibocerosOracleSubDJoinSDK20261102',timeout=900)


if __name__=='__main__':main()

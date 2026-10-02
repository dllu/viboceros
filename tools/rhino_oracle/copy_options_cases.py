"""Independent command starts, option edits, completions, and cancellations."""
import json


def request():
    sequence = [
        ('RememberCopyOptions', True, 'Complete'),
        ('ExtractSrf', False, 'Complete'),
        ('ExtractSrf', True, 'Complete'),
        ('ExtractSrf', None, 'Complete'),
        ('ExtractSrf', False, 'Cancel'),
        ('ExtractSrf', None, 'Complete'),
        ('Rotate', True, 'Complete'),
        ('Rotate', False, 'Cancel'),
        ('Rotate', None, 'Complete'),
        ('Rotate', None, 'Complete'),
        ('Scale', None, 'Complete'),
        ('Mirror', False, 'Complete'),
        ('Mirror', True, 'Cancel'),
        ('Mirror', None, 'Complete'),
        ('Scale', True, 'Cancel'),
        ('Scale', None, 'Complete'),
        ('Scale', True, 'Complete'),
        ('Scale', None, 'Complete'),
        ('RememberCopyOptions', False, 'Complete'),
        ('ExtractSrf', True, 'Complete'),
        ('ExtractSrf', None, 'Complete'),
        ('Rotate', True, 'Complete'),
        ('Rotate', False, 'Cancel'),
        ('Rotate', None, 'Complete'),
        ('Rotate', None, 'Complete'),
        ('Mirror', False, 'Complete'),
        ('Mirror', None, 'Complete'),
        ('Scale', True, 'Cancel'),
        ('Scale', None, 'Complete'),
        ('RememberCopyOptions', True, 'Complete'),
        ('ExtractSrf', None, 'Complete'),
        ('Rotate', None, 'Complete'),
        ('Mirror', None, 'Complete'),
        ('Scale', None, 'Complete'),
        ('RememberCopyOptions', False, 'Complete'),
        ('ExtractSrf', True, 'Cancel'),
        ('RememberCopyOptions', True, 'Complete'),
        ('ExtractSrf', None, 'Complete'),
        ('RememberCopyOptions', False, 'Complete'),
        ('Rotate', True, 'Complete'),
        ('RememberCopyOptions', True, 'Complete'),
        ('Rotate', None, 'Complete'),
        ('RememberCopyOptions', False, 'Complete'),
        ('Mirror', False, 'Complete'),
        ('RememberCopyOptions', True, 'Complete'),
        ('Mirror', None, 'Complete'),
        ('RememberCopyOptions', False, 'Complete'),
        ('Rotate', True, 'Cancel'),
        ('RememberCopyOptions', True, 'Complete'),
        ('Rotate', None, 'Complete'),
    ]
    return dict(
        protocol_version=1,
        iterations=1,
        operations=[dict(
            op='copy_options_command',
            id='copy-options-sequence',
            sources=[dict(brep=dict(source=dict(type='box', min=[0., 0., 0.], max=[2., 3., 4.])))],
            steps=[dict(command=name, copy=choice, finish=finish) for name, choice, finish in sequence],
        )],
    )


if __name__ == '__main__':
    print(json.dumps(request(), indent=2))

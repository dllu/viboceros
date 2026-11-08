"""Physical Escape restricted to the private Shell worker window."""
from .chamfer_point_picker import ChamferPointPicker


class ShellPointPicker(ChamferPointPicker):
    key_prefix = '@shell-escape:'

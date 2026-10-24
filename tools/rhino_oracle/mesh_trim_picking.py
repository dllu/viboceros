"""Allow the owned trim getter to open before each physical region pick."""
import time
from .component_picking import ComponentPicker


class MeshTrimPicker(ComponentPicker):
    def send_input(self, name, x, y, window):
        if name.startswith('@component-click:'):
            time.sleep(1.)
        return super().send_input(name, x, y, window)

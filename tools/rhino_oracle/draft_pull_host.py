"""Bounded physical edge picks in the owned private Rhino window."""
import time
from .group_picking import IdlePicker
from .component_picking import ComponentPicker
from .client import OracleProtocolError


class DraftPullPicker(IdlePicker):
    def __init__(self,request):
        super().__init__()
        self.markers={'@draft-pull-edge:'+op['id'] for op in request['operations']
                      if op.get('op')=='draft_pull_direction' and op['curve']=='BoxEdge' and not op['cancel']}

    def send_input(self,name,x,y,window):
        if name not in self.markers:raise OracleProtocolError('foreign draft-direction input marker')
        result=super().send_input(name,x,y,window)
        for unused in range(20):
            time.sleep(.1)
            if ComponentPicker().send_input('@component-menu:pull-edge:First','1','1',window):break
        from PIL import ImageGrab
        ImageGrab.grab().save('/tmp/viboceros-pull-edge-'+name.split(':',1)[1]+'.png')
        return result

    def record_diagnostics(self,response):
        if self.seen!=self.markers:raise OracleProtocolError('incomplete physical draft edge input')

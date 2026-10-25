"""Optional interface settings and cleanup across host failures."""
from types import SimpleNamespace
import unittest
from unittest.mock import Mock,patch
from . import interface_commands_probe as probe

class InterfaceCommandsTests(unittest.TestCase):
    def test_subd_schema_validation_does_not_access_the_host(self):
        base=dict(grid_snap=True,osnap=False,smart_track=False,active_viewport=0,
            display_modes=['Wireframe']*4,commands=['SubDDisplayToggle'])
        for extra in [{},{'subd_control_net':None},{'subd_control_net':1},{'subd_control_net':'Flat'}]:
            with self.subTest(extra=extra),self.assertRaises(ValueError):
                probe.run(dict(base,**extra),{'_interface_script':probe.script})
        for command in ['SubDDisplayToggle _Delete','SubDDisplayToggle On','SubDDisplayToggle\n_Delete']:
            with self.assertRaises(ValueError):probe.script(command)
        self.assertEqual(probe.script("'-_sUbDdIsPlAyToGgLe"),'_SubDDisplayToggle')

    def test_subd_and_other_settings_restore_even_when_cleanup_fails(self):
        for initial in [False,True]:
            for failure in [None,'command','record','cancel','mesh','both','initialize']:
                with self.subTest(initial=initial,failure=failure):
                    self.run_case(initial,failure)

    def run_case(self,initial,failure):
        class Settings(SimpleNamespace):
            def GetCurrentState(self):return vars(self).copy()
            def UpdateFromState(self,state):self.__dict__.clear();self.__dict__.update(state)
        class Document:
            def __init__(self):self.appearance=not initial;self.initialized=False
            @property
            def SubDAppearance(self):return self.appearance
            @SubDAppearance.setter
            def SubDAppearance(self,value):
                self.appearance=value
                if failure=='initialize' and not self.initialized:
                    self.initialized=True
                    raise ValueError('initialize failure')
        aid=Settings(GridSnap=False,Osnap=True,OtherSetting='original')
        track=Settings(UseSmartTrack=True,OtherSetting=42)
        doc=Document();views=[SimpleNamespace(ActiveViewportID=i,ActiveViewport=SimpleNamespace(DisplayMode=object())) for i in range(4)]
        doc.Views=SimpleNamespace(ActiveView=views[1],GetViewList=lambda a,b:views)
        modes=[v.ActiveViewport.DisplayMode for v in views]
        original_aid=aid.GetCurrentState();original_track=track.GetCurrentState()
        mesh=SimpleNamespace(enabled=True,calls=0)
        def set_mesh(value,host):
            mesh.enabled=value;mesh.calls+=1
            if mesh.calls==2 and failure in ['mesh','both']:raise ValueError('mesh failure')
        def cancel(*args):
            if failure in ['cancel','both']:raise ValueError('cancel failure')
        rhino=SimpleNamespace(RhinoDoc=SimpleNamespace(ActiveDoc=doc),
            Geometry=SimpleNamespace(SubDComponentLocation=SimpleNamespace(Surface=False,ControlNet=True)),
            Display=SimpleNamespace(DisplayModeDescription=SimpleNamespace(FindByName=lambda n:SimpleNamespace(EnglishName=n))),
            ApplicationSettings=SimpleNamespace(ModelAidSettings=aid,SmartTrackSettings=track),
            RhinoApp=SimpleNamespace(RunScript=Mock(side_effect=cancel)))
        host={'Rhino':rhino,'_interface_script':probe.script,'_record_progress':lambda message:None}
        calls=[0]
        def record(*args):
            calls[0]+=1
            if failure=='record' and calls[0]==2:raise ValueError('record failure')
            return probe.state(*args,host)
        def execute(command,verify):
            if command=='_SubDDisplayToggle':doc.SubDAppearance=not doc.SubDAppearance
            else:aid.GridSnap=not aid.GridSnap
            aid.OtherSetting='changed';track.OtherSetting=99
            if failure=='command':raise ValueError('command failure')
            return True
        host.update(_interface_state=record,_run_surface_script=execute)
        operation=dict(grid_snap=True,osnap=False,smart_track=False,snap_to_meshes=False,
            subd_control_net=initial,active_viewport=2,display_modes=['Wireframe']*4,
            commands=['SubDDisplayToggle','Snap','SubDDisplayToggle'])
        api=SimpleNamespace(current=lambda host:mesh.enabled,set_enabled=set_mesh)
        with patch.dict('sys.modules',{'mesh_snap_settings_probe':api}):
            if failure:
                expected='cancel' if failure=='both' else failure
                with self.assertRaisesRegex(ValueError,expected+' failure'):probe.run(operation,host)
            else:
                result,_=probe.run(operation,host)
                self.assertEqual([r['subd_control_net'] for r in result['states']],[initial,not initial,not initial,initial])
        self.assertEqual(doc.SubDAppearance,not initial)
        self.assertEqual(aid.GetCurrentState(),original_aid);self.assertEqual(track.GetCurrentState(),original_track)
        self.assertEqual([v.ActiveViewport.DisplayMode for v in views],modes)
        self.assertIs(doc.Views.ActiveView,views[1]);self.assertTrue(mesh.enabled)

"""Plane fixture admission and SDK ownership checks without a live Rhino host."""
import copy
import sys
import unittest
from types import SimpleNamespace as NS
from unittest.mock import Mock, patch
from tools.rhino_oracle import iplane_probe as probe


def operation():
    return dict(op='iplane',id='test',sources=[dict(type='line',start=[0,0,-1],end=[0,0,1])],origin=[0,0,0],normal=[0,0,1])


class IPlaneProbeTests(unittest.TestCase):
    def test_invalid_planes_and_source_sets_decline_before_sdk_access(self):
        probe.validate(operation())
        for key,value in [('normal',[0,0,0]),('normal',[0,0,float('inf')]),('origin',[True,0,0]),('origin',[0,0]),('sources',[]),('sources',[{}]*65),('sources',[dict(type='point',point=[0,0,0])])]:
            op=operation();op[key]=value
            with self.subTest(key=key,value=value),self.assertRaises(ValueError): probe.validate(op)
        op=operation();op['extra']=True
        with self.assertRaises(ValueError):probe.validate(op)

    def test_curve_contacts_overlaps_and_joined_results_dispose_on_failures(self):
        for failure in (None,'allocation','sdk','trim','sampling'):
            class Curve:
                JoinCurves=Mock()
            class Events(list):
                Dispose=Mock()
            source=Curve();source.Dispose=Mock();source.Trim=Mock()
            overlap=NS(Dispose=Mock())
            joined=NS(Dispose=Mock(),DivideByCount=Mock(return_value=None if failure=='sampling' else [0.,1.]),PointAt=lambda t:[t,0.,0.],GetLength=lambda:1.,IsClosed=False)
            source.Trim.return_value=None if failure=='trim' else overlap
            Curve.JoinCurves.return_value=[joined]
            events=Events([NS(IsPoint=True,PointA=[0.,0.,0.]),NS(IsPoint=False,IsOverlap=True,OverlapA=[0.,1.])])
            intersection=NS(CurvePlane=Mock(return_value=events,side_effect=ValueError('sdk') if failure=='sdk' else None))
            geometry=NS(Curve=Curve,Mesh=type('Mesh',(),{}),Surface=type('Surface',(),{}),Brep=type('Brep',(),{}),Plane=lambda *args:NS(IsValid=True),Intersect=NS(Intersection=intersection))
            op=operation()
            if failure=='allocation':op['sources'].append(copy.deepcopy(op['sources'][0]))
            allocate=Mock(side_effect=[source,ValueError('allocation')] if failure=='allocation' else None,return_value=source)
            host=dict(_vector=lambda v:NS(Unitize=lambda:True),_point=lambda p:p,_xyz=lambda p:p,_object_source=allocate,_measure=lambda n,fn:(fn(),0))
            with self.subTest(failure=failure),patch.dict(sys.modules,{'Rhino':NS(Geometry=geometry)}):
                if failure:
                    with self.assertRaises(ValueError):probe.run(op,1,dict(absolute=1e-7),host)
                else:
                    result,_=probe.run(op,1,dict(absolute=1e-7),host)
                    self.assertEqual(result['points'],[[0.,0.,0.]])
                    self.assertEqual(result['curves'][0]['samples'],[[0.,0.,0.],[1.,0.,0.]])
            source.Dispose.assert_called_once()
            self.assertEqual(events.Dispose.call_count,int(failure not in ('allocation','sdk')))
            self.assertEqual(overlap.Dispose.call_count,int(failure in (None,'sampling')))
            self.assertEqual(joined.Dispose.call_count,int(failure in (None,'sampling')))

    def test_surface_conversion_and_partial_results_are_disposed(self):
        for success in (True,False):
            class Curve:JoinCurves=Mock(return_value=[])
            class Surface:pass
            source=Surface();source.Dispose=Mock()
            brep=NS(Dispose=Mock());source.ToBrep=Mock(return_value=brep)
            curve=NS(Dispose=Mock(),DivideByCount=lambda *args:[0.,1.],PointAt=lambda t:[t,0.,0.],GetLength=lambda:1.,IsClosed=False)
            joined=NS(Dispose=Mock(),DivideByCount=lambda *args:[0.,1.],PointAt=lambda t:[t,0.,0.],GetLength=lambda:1.,IsClosed=False)
            Curve.JoinCurves.return_value=[joined]
            geometry=NS(Curve=Curve,Surface=Surface,Mesh=type('Mesh',(),{}),Brep=type('Brep',(),{}),Plane=lambda *args:NS(IsValid=True),Intersect=NS(Intersection=NS(BrepPlane=lambda *args:(success,[curve],[]))))
            host=dict(_vector=lambda v:NS(Unitize=lambda:True),_point=lambda p:p,_xyz=lambda p:p,_object_source=lambda *args:source,_measure=lambda n,fn:(fn(),0))
            with self.subTest(success=success),patch.dict(sys.modules,{'Rhino':NS(Geometry=geometry)}):
                if success:probe.run(operation(),1,dict(absolute=1e-7),host)
                else:
                    with self.assertRaises(ValueError):probe.run(operation(),1,dict(absolute=1e-7),host)
            source.Dispose.assert_called_once();brep.Dispose.assert_called_once();curve.Dispose.assert_called_once()
            self.assertEqual(joined.Dispose.call_count,int(success))

    def test_mesh_section_conversion_is_disposed_when_sampling_fails(self):
        class Mesh:pass
        class Curve:
            JoinCurves=Mock(return_value=[])
        source=Mesh();source.Dispose=Mock()
        converted=NS(Dispose=Mock())
        joined=NS(Dispose=Mock(),DivideByCount=lambda *args:None)
        Curve.JoinCurves.return_value=[joined]
        polyline=NS(ToNurbsCurve=lambda:converted)
        mesh_plane=Mock(return_value=[polyline])
        geometry=NS(Curve=Curve,Mesh=Mesh,Surface=type('Surface',(),{}),Brep=type('Brep',(),{}),Plane=lambda *args:NS(IsValid=True),Intersect=NS(Intersection=NS(MeshPlane=mesh_plane)))
        host=dict(_vector=lambda v:NS(Unitize=lambda:True),_point=lambda p:p,_xyz=lambda p:p,_object_source=lambda *args:source,_measure=lambda n,fn:(fn(),0))
        with patch.dict(sys.modules,{'Rhino':NS(Geometry=geometry)}),self.assertRaises(ValueError):probe.run(operation(),1,dict(absolute=1e-7),host)
        self.assertEqual(mesh_plane.call_args.args[1],None)
        self.assertEqual(mesh_plane.call_args.args[3],1e-7)
        source.Dispose.assert_called_once();converted.Dispose.assert_called_once();joined.Dispose.assert_called_once()

    def test_client_rejects_invalid_plane_before_launch(self):
        import os
        import tempfile
        from tools.rhino_oracle.client import OracleClient
        with tempfile.NamedTemporaryFile() as launcher,patch.dict(os.environ,{'DISPLAY':':198','VIBOCEROS_ORACLE_HEADLESS':':198'}):
            client=OracleClient(launcher=launcher.name)
            op=operation();op['normal']=[0,0,0]
            with self.assertRaisesRegex(ValueError,'nonzero plane normal'):
                client.run_rhino(dict(protocol_version=1,iterations=1,operations=[op]))

    def test_closed_sdk_sample_sequence_keeps_the_seam_once(self):
        class Curve:
            JoinCurves=Mock()
        source=Curve();source.Dispose=Mock()
        class Events(list):
            Dispose=Mock()
        overlap=NS(Dispose=Mock());source.Trim=lambda domain:overlap
        joined=NS(Dispose=Mock(),DivideByCount=lambda *args:list(range(33)),PointAt=lambda i:[i%32,0.,0.],GetLength=lambda:8.,IsClosed=True)
        Curve.JoinCurves.return_value=[joined]
        events=Events([NS(IsPoint=False,IsOverlap=True,OverlapA=[0.,1.])])
        geometry=NS(Curve=Curve,Mesh=type('Mesh',(),{}),Surface=type('Surface',(),{}),Brep=type('Brep',(),{}),Plane=lambda *args:NS(IsValid=True),Intersect=NS(Intersection=NS(CurvePlane=lambda *args:events)))
        host=dict(_vector=lambda v:NS(Unitize=lambda:True),_point=lambda p:p,_xyz=lambda p:p,_object_source=lambda *args:source,_measure=lambda n,fn:(fn(),0))
        with patch.dict(sys.modules,{'Rhino':NS(Geometry=geometry)}):
            result,_=probe.run(operation(),1,dict(absolute=1e-7),host)
        samples=result['curves'][0]['samples']
        self.assertEqual(len(samples),32)
        self.assertEqual(samples.count(samples[0]),1)

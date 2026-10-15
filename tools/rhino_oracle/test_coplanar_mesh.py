"""Public SDK mesh routing/ownership mocks, without licensed parity claims."""
import copy
from types import SimpleNamespace as NS
from unittest import TestCase,mock
from .coplanar_faces_probe import run,validate
from .coplanar_mesh_probe import request
class Counted(list):
    @property
    def Count(self):return len(self)
class MeshTests(TestCase):
    def host(self,closed=False):
        class Mesh:pass
        source=Mesh();source.IsValid=True;source.IsClosed=closed;source.Faces=Counted([0,1]);source.Ngons=Counted();source.Dispose=mock.Mock()
        output=mock.Mock(IsValid=True,IsClosed=closed);output.Faces=Counted([0]);output.Ngons=Counted([NS(FaceIndexList=lambda:[0])]);output.Vertices=Counted(range(4))
        output.MergeAllCoplanarFaces.return_value=True;output.GetBoundingBox.return_value=NS(Min=[0,0,0],Max=[4,3,0])
        source.DuplicateMesh=mock.Mock(return_value=output)
        area=mock.Mock(Area=12.);volume=mock.Mock(Volume=24.)
        G=NS(Mesh=Mesh,AreaMassProperties=NS(Compute=mock.Mock(return_value=area)),VolumeMassProperties=NS(Compute=mock.Mock(return_value=volume)))
        host={'Rhino':NS(Geometry=G),'_object_source':lambda *args:source,'_measure':lambda n,f:(f(),0),'_xyz':lambda p:p,'_polygon_mesh_value':mock.Mock(return_value={'faces':[[0,1,2,3]],'vertices':[]})}
        return host,source,output,area,volume
    def test_strict_mesh_recipes(self):
        for op in request()['operations']:validate(op)
        op=copy.deepcopy(request()['operations'][0]);op['source']['faces']=[[True,1,2]]
        with self.assertRaises(ValueError):validate(op)
    def test_mesh_sdk_clone_tolerances_ngons_and_disposal(self):
        for closed in (False,True):
            host,source,output,area,volume=self.host(closed)
            value,_=run(request()['operations'][0],host,1,request()['tolerance'])
            output.MergeAllCoplanarFaces.assert_called_once_with(1e-9,1e-8);source.DuplicateMesh.assert_called_once()
            self.assertEqual(value['logical_face_count'],1);self.assertEqual(value['removed_faces'],1);self.assertEqual(value['ngon_face_counts'],[1]);self.assertEqual(value['volume'],24. if closed else None)
            host['_polygon_mesh_value'].assert_called_once_with(output)
            for obj in (source,output,area):obj.Dispose.assert_called_once()
            if closed:volume.Dispose.assert_called_once()
    def test_noops_and_failure_cleanup(self):
        host,source,output,_,_=self.host();output.Faces=source.Faces;output.Ngons=Counted();output.MergeAllCoplanarFaces.return_value=False
        value,_=run(request()['operations'][0],host,1,request()['tolerance']);self.assertEqual(value['removed_faces'],0)
        for stage in ('copy','native','invalid','closure','area','volume','false_change','bounds'):
            host,source,output,area,volume=self.host(True)
            if stage=='copy':source.DuplicateMesh.return_value=None
            elif stage=='native':output.MergeAllCoplanarFaces.side_effect=ValueError('failed')
            elif stage=='invalid':output.IsValid=False
            elif stage=='closure':output.IsClosed=False
            elif stage=='area':host['Rhino'].Geometry.AreaMassProperties.Compute.return_value=None
            elif stage=='volume':host['Rhino'].Geometry.VolumeMassProperties.Compute.return_value=None
            elif stage=='false_change':output.MergeAllCoplanarFaces.return_value=False
            else:output.GetBoundingBox.return_value=NS(Min=[float('nan'),0,0],Max=[4,3,0])
            with self.subTest(stage=stage),self.assertRaises(ValueError):run(request()['operations'][0],host,1,request()['tolerance'])
            source.Dispose.assert_called_once()
            if stage!='copy':output.Dispose.assert_called_once()
            if stage in ('volume','bounds'):area.Dispose.assert_called_once()

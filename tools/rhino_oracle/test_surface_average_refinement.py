"""Keep rejected models, invalid public calls and Native failures visible."""
import copy,gzip,hashlib,json,shutil,tempfile,unittest
from pathlib import Path
from .surface_average_refinement_evidence import ROOT,PROOF,collect,composition,prototypes,complete
from .report_surface_average_refinement import build

class SurfaceAverageRefinementTests(unittest.TestCase):
    def test_catalog_reproduces_and_does_not_qualify_prototypes(self):
        value=json.loads((ROOT/'docs/surface-average-refinement-investigation.json').read_bytes())
        self.assertEqual(value,build())
        self.assertEqual((value['native_primary_observations'],value['diagnostic_composed_calls'],value['invalid_diagnostic_calls']),(102,40,4))
        self.assertFalse(value['averaged_refinement_qualified'])
        self.assertEqual({k:v['matched_cases']for k,v in value['frozen_prototypes'].items()},{'direct':25,'inverse':24})
        self.assertTrue(all(not v['qualified']for v in value['frozen_prototypes'].values()))
        self.assertEqual(len(value['inverse_ambiguity_examples']),6)
        self.assertGreater(value['curvature_contact_example']['relative_difference'],.6)
        for p,digest in value['evidence_sha256'].items():self.assertEqual(hashlib.sha256((ROOT/p).read_bytes()).hexdigest(),digest,p)
    def test_missing_duplicate_and_unowned_launches_are_rejected(self):
        with tempfile.TemporaryDirectory()as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);path=proof/'captures.json';rows=json.loads(path.read_bytes())
            variants=[rows[:-1],rows+rows[-1:]]
            for key,bad in [('exit_status',None),('command',['desktop-launcher']),('observation_sha256','0'*64)]:
                changed=copy.deepcopy(rows);changed[0][key]=bad;variants.append(changed)
            for changed in variants:
                path.write_text(json.dumps(changed))
                with self.assertRaises(ValueError):collect(proof)
    def test_failed_capture_cannot_be_erased_or_reclassified(self):
        with tempfile.TemporaryDirectory()as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);path=proof/'attempts.json';rows=json.loads(path.read_bytes())
            changed=copy.deepcopy(rows);changed[0]['exit_status']=0
            for variant in [[],rows+rows,changed]:
                path.write_text(json.dumps(variant))
                with self.assertRaises(ValueError):collect(proof)
    def test_composition_rejects_missing_steps_wrong_flags_and_disguised_failures(self):
        rows,_=collect();value=rows['average_refine_steps_polynomial_tangency_01']
        variants=[]
        changed=copy.deepcopy(value);changed['average_pass_audit'].pop();variants.append(changed)
        changed=copy.deepcopy(value);changed['average_pass_audit'][0]['value']['prescribed_case']['reverse_match']=False;variants.append(changed)
        changed=copy.deepcopy(value);changed['average_pass_audit'][1]['error']='unrelated helper failure';variants.append(changed)
        for changed in variants:
            with self.assertRaises(ValueError):composition(changed)
        changed=copy.deepcopy(value);changed['configured_settings']['distance']=0.
        with self.assertRaises(ValueError):complete(changed)
    def test_prototypes_cannot_hide_failed_rows_or_missing_sources(self):
        with tempfile.TemporaryDirectory()as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);path=proof/'prototypes/direct.json';original=path.read_bytes();value=json.loads(original)
            for key,bad in [('exit_status',0),('sources',{}),('baseline_revision','0'*40)]:
                changed=copy.deepcopy(value);changed[key]=bad;path.write_text(json.dumps(changed))
                with self.assertRaises(ValueError):prototypes(proof)
            path.write_bytes(original);path=proof/'prototypes/direct.log.gz';lines=gzip.decompress(path.read_bytes()).decode().splitlines()
            i=next(i for i,s in enumerate(lines)if s.startswith('AVERAGE_REFINEMENT '));row=json.loads(lines[i].split(' ',1)[1]);bad=lines[:];bad[i]='AVERAGE_REFINEMENT '+json.dumps(dict(row,maximum_geometry_error=1e-3))
            for changed in [lines[:i]+lines[i+1:],lines+[lines[i]],bad]:
                path.write_bytes(gzip.compress('\n'.join(changed).encode(),mtime=0))
                with self.assertRaises(ValueError):prototypes(proof)

if __name__=='__main__':unittest.main()

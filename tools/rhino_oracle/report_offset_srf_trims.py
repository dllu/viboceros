"""Pin finite trimmed-offset comparisons without changing earlier qualifications."""
import argparse
import gzip
import json
import re
import subprocess
from .check_offset_srf_trims import ROOT,PROOF,commands
from .offset_srf_trims_evidence import collect,DIAGNOSTICS,FAMILIES
from .report_offset_srf_tolerance import requested, gate, result, sha

BASELINE='da985d13cb206ee07b8cf18e6725e2d02c9de103'

def replay_text(path):
    text=gzip.decompress(path.read_bytes()).decode()
    if re.search(r'^test result: (?!ok\.)',text,re.MULTILINE) or re.search(
        r'^test result: ok\. [1-9][0-9]* passed; 0 failed; 0 ignored;',text,re.MULTILINE)is None:
        raise ValueError('failed replay')
    return text

def maximum_error(cases, metric):
    return max(c.get(stage+'_'+mode+'_'+metric,0.) for c in cases.values()
        for stage in ['command','ui'] for mode in ['portable','native'])

def comparisons(proof=PROOF):
    rows,captures,counts,paths=collect(proof)
    names=set(rows)-set(DIAGNOSTICS)
    trims=set().union(*FAMILIES.values())-set(DIAGNOSTICS)
    cases={k:dict(native_result=result(v),qualified=k in names) for k,v in rows.items()}
    for k,reason in DIAGNOSTICS.items(): cases[k]['reason']=reason
    for k in names:
        fit=requested(rows[k]); curved=rows[k]['prescribed_case']['curves'][0]['type']!='normal_surface'
        cases[k].update(requested_tolerance=fit,epsilon=max(1e-5,2*fit) if curved else 1e-6,
            native_reference_tolerance=max(fit,rows[k]['model_tolerance']['absolute']))
    for mode in ['portable','native']:
        expected={k for k in names if result(rows[k])!='Success' or (k!='offset_th_rational' if mode=='native' else rows[k]['prescribed_case']['curves'][0]['type']=='normal_surface')}
        for stage in ['command','ui']:
            path=proof/(stage+'-'+mode+'.log.gz'); paths.add(path)
            text=replay_text(path)
            if (stage,mode)==('command','native'):
                extra=proof/'trim-command-native.log.gz'; paths.add(extra)
                text+='\n'+replay_text(extra)
            seen=set()
            for prefix,group in [('TOLERANCE_OFFSET',names-trims),('TRIM_OFFSET',trims)]:
                prefix+='_'+stage.upper()
                for line in text.splitlines():
                    if not line.startswith(prefix+' '): continue
                    row=json.loads(line.split(' ',1)[1]); k=row['id']
                    if k in seen or k not in expected & group: raise ValueError('incomplete or duplicate comparison')
                    gate(row['maximum_geometry_error'],cases[k]['epsilon'])
                    gate(row['maximum_field_error'],cases[k]['requested_tolerance']+1e-10)
                    gate(row['native_maximum_field_error'],float('inf'))
                    if row['native_reference_tolerance']!=cases[k]['native_reference_tolerance']: raise ValueError('reference admission budget')
                    if row['archives']!=1 or row['changed']is not rows[k]['history_changed'] or row['success']is not(result(rows[k])=='Success'): raise ValueError('history or archive gate')
                    seen.add(k)
                    for key in ['maximum_geometry_error','maximum_field_error','native_maximum_field_error']: cases[k][stage+'_'+mode+'_'+key]=row[key]
                declined=[line.split(' ',1)[1] for line in text.splitlines() if line.startswith(prefix+'_DECLINED ')]
                marker=prefix+'_COUNT '+str(len(group & expected))+' DECLINED '+str(len(group-expected))
                if set(declined)!=group-expected or len(declined)!=len(group-expected) or text.count(marker)!=1: raise ValueError('incomplete or duplicate comparison')
                for k in group-expected: cases[k][stage+'_'+mode+'_atomic_decline']=True
            if seen!=expected: raise ValueError('incomplete or duplicate comparison')
    return cases,captures,counts,paths

def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '')is None: raise ValueError('full implementation revision required')
    git=lambda *args:subprocess.check_output(['git',*args],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}'); git('merge-base','--is-ancestor',BASELINE,revision)
    checks=json.loads((proof/'checks.json').read_bytes())
    if [(c['name'],c['command']) for c in checks]!=list(commands()) or any(c['status'] for c in checks): raise ValueError('incomplete checks')
    cases,captures,counts,paths=comparisons(proof)
    paths.update(p for p in proof.iterdir() if p.is_file())
    changed=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    if not {'crates/viboceros-command/src/offset_surface/native/trimmed.rs',
        'crates/viboceros-geometry/src/brep/surface_edit/isocurve.rs',
        'crates/viboceros-smlib/native/brep_export.cpp',
        'crates/viboceros-smlib/src/native/shape.rs'}.issubset(changed): raise ValueError('missing implementation')
    return dict(scope='Non-Loose OffsetSrf open single-face trimmed sheets.',
        baseline_revision=BASELINE,implementation_revision=revision,full_native_parity=False,
        continuous_true_offset_bound=False,performance_qualified=False,**counts,
        qualified_successful=31,qualified_cancelled=5,portable_supported=22,native_supported=35,
        archive_round_trips=114,maximum_geometry_difference=maximum_error(cases,'maximum_geometry_error'),
        maximum_product_sampled_field_error=maximum_error(cases,'maximum_field_error'),
        maximum_native_sampled_field_error=maximum_error(cases,'native_maximum_field_error'),
        execution='Both new licensed Rhino 8 captures and the failed import attempt used private Xvfb sessions. All thirty-nine outcomes remain, including a hidden-option diagnostic and the earlier two input diagnostics. Rust checks open no windows.',
        implementation='Offset the untrimmed support surface using one quarter of the requested tolerance for its native fit, then preserve original UV loops and face orientation. Export only the support; restrict extended domains without reparameterization. Rebuild spatial trim images with continuous surface/curve certificates at one quarter of document tolerance. Piecewise axis-aligned UV trims retain their original parameter clock and rational isocurves; other trims use bounded fitting.',
        comparison='Authored inputs and independent source-normal displacements; native output coefficients are expectations only. Reference B-reps are admitted at max(document tolerance, requested fit tolerance), recorded per case; Rhino edge/UV fits can exceed document tolerance. Product validation retains document tolerance. Command and UI replays check geometry, metadata, identity, selection, pure previews, cancellation, actual Undo/Redo and SaveAs/Open3DM. Geometry uses bidirectional finite valid trimmed-face and boundary stations, epsilon 1e-6 for planes or max(1e-5,2*requested) for curved refits. Product source-normal station residuals must be <=requested+1e-10; native residuals are recorded separately.',
        limits=['General curved offsets require native-smlib. Portable builds atomically decline fourteen successful curved cases; cancellation remains available.',
            'Finite comparisons sample faces and UV-trim images; native model-space edge parity is not separately qualified.',
            'The nonuniform rational curvature-break case still declines atomically. Its native geometry parity remains unqualified.',
            'This repair covers open single-face trimmed sheets without G2 breaks, including separate BothSides skins. Trimmed supports with curvature breaks decline atomically; general trimmed solids remain on the existing native path.',
            'Trim-image certificates do not certify continuous distance to the ideal offset. Arbitrary singularities, collapse, self-intersections, precision finer than 1e-7 and comparative performance remain unqualified.'],
        cases=[dict(id=k,**cases[k]) for k in sorted(cases)],captures=captures,
        checks=[dict(name=c['name'],status=c['status']) for c in checks],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p)) for p in changed},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in sorted(paths)})

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__); parser.add_argument('--implementation-revision',required=True); args=parser.parse_args()
    (ROOT/'docs/offset-srf-trims-validation.json').write_text(json.dumps(build(PROOF,args.implementation_revision),indent=2,allow_nan=False)+'\n')

"""Compile the current C ABI bridge probe against a completed native Cargo build.

--asan instruments the bridge and driver only. The reused kernel/helper archives
remain uninstrumented; this is not full-kernel sanitizer coverage.
"""
import argparse,os,subprocess
from pathlib import Path

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--native-build',type=Path,required=True)
    p.add_argument('--output-dir',type=Path,required=True)
    p.add_argument('--asan',action='store_true')
    a=p.parse_args();root=Path(__file__).resolve().parents[2];build=a.native_build.resolve();out=a.output_dir.resolve();out.mkdir(parents=True,exist_ok=True)
    libs=[build/('lib'+n+'.a')for n in ('viboceros_smlib','SM_API','SMLib')]
    if not all(p.is_file()for p in libs):p.error('native-build must contain the completed native archives')
    native=root/'crates/viboceros-smlib/native';upstream=root/'third_party/usd-brep/source'
    flags=['-std=c++17','-O1','-g','-fno-omit-frame-pointer','-DNOMINMAX','-DSM_GRAPHICS_CALLBACKS=1','-DVIBOCEROS_CURVE_INTERSECTION_DISTANCE_PATCH=1','-I'+str(native),'-I'+str(build/'usd-brep-patched'),'-isystem',str(upstream/'SMLib/inc'),'-isystem',str(upstream/'SM_API/inc')]
    if a.asan:flags+=['-fsanitize=address']
    obj=out/'bridge.o';exe=out/'curve-distance-probe'
    subprocess.run(['c++',*flags,'-c',str(native/'bridge.cpp'),'-o',str(obj)],check=True)
    subprocess.run(['c++',*flags,str(root/'tools/usd_brep/curve_distance_probe.cpp'),str(obj),'-Wl,--start-group',*map(str,libs),'-Wl,--end-group','-ltbb','-ltbbmalloc','-o',str(exe)],check=True)
    env=os.environ.copy()
    if a.asan:env['ASAN_OPTIONS']='detect_leaks=0:halt_on_error=1'
    subprocess.run([str(exe)],env=env,check=True)
if __name__=='__main__':main()

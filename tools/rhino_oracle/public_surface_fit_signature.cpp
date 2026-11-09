// ABI name generation for the documented public Rhino C++ SDK function only.
class ON_Surface;
__declspec(dllimport) ON_Surface* RhinoFitSurface(const ON_Surface&, int, int, double, double*);
auto fit = &RhinoFitSurface;

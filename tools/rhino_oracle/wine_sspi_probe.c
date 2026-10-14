/* Public Windows loader diagnostics; no Rhino or authentication calls. */
#include <windows.h>
#include <stdio.h>

int main(void)
{
    const char *libraries[] = {"sspicli.dll", "secur32.dll"};
    const char *symbols[] = {
        "AcquireCredentialsHandleW", "FreeCredentialsHandle",
        "InitializeSecurityContextW"
    };
    int failed = 0;
    printf("VIBOCEROS_SSPI {\"libraries\":[");
    for (int i = 0; i < 2; ++i) {
        SetLastError(0);
        HMODULE module = LoadLibraryA(libraries[i]);
        DWORD error = module ? 0 : GetLastError();
        printf("%s{\"name\":\"%s\",\"loaded\":%s,\"error\":%lu,\"symbols\":[",
               i ? "," : "", libraries[i], module ? "true" : "false", error);
        failed |= module == NULL;
        for (int j = 0; j < 3; ++j) {
            SetLastError(0);
            FARPROC symbol = module ? GetProcAddress(module, symbols[j]) : NULL;
            error = module && !symbol ? GetLastError() : 0;
            printf("%s{\"name\":\"%s\",\"attempted\":%s,\"resolved\":%s,\"error\":%lu}",
                   j ? "," : "", symbols[j], module ? "true" : "false",
                   symbol ? "true" : "false", error);
            failed |= symbol == NULL;
        }
        printf("]}");
        if (module) FreeLibrary(module);
    }
    printf("]}\n");
    return failed;
}

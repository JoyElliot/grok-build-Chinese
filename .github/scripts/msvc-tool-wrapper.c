/* CI-only native tool wrapper: preserve MSVC names and the Win32 command limit. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wchar.h>
#include <stdio.h>
#include <stdlib.h>
#include "msvc-wrapper-config.h"

static DWORD fail(const wchar_t *operation) {
    DWORD code = GetLastError();
    fwprintf(stderr, L"MSVC wrapper: %ls failed (%lu)\n", operation, code);
    return code ? code : ERROR_INVALID_DATA;
}

static int load_environment(void) {
    FILE *file = NULL;
    if (_wfopen_s(&file, WRAPPER_ENV_FILE, L"rb") || !file) return 0;
    if (fseek(file, 0, SEEK_END)) { fclose(file); return 0; }
    long size = ftell(file);
    if (size <= 0 || size > 1048576 || (size % 2)) { fclose(file); return 0; }
    rewind(file);
    wchar_t *data = calloc((size_t)size / 2 + 1, sizeof(wchar_t));
    if (!data) { fclose(file); return 0; }
    if (fread(data, 1, (size_t)size, file) != (size_t)size) { free(data); fclose(file); return 0; }
    fclose(file);
    wchar_t *line = data + (data[0] == 0xfeff);
    int ok = 1;
    while (*line) {
        wchar_t *end = wcschr(line, L'\n');
        if (end) *end = 0;
        size_t length = wcslen(line);
        if (length && line[length - 1] == L'\r') line[length - 1] = 0;
        wchar_t *equals = wcschr(line, L'=');
        if (!equals || equals == line) { ok = 0; break; }
        *equals = 0;
        if (!SetEnvironmentVariableW(line, equals + 1)) { ok = 0; break; }
        if (!end) break;
        line = end + 1;
    }
    free(data);
    return ok;
}

int wmain(void) {
    wchar_t self[32768];
    DWORD n = GetModuleFileNameW(NULL, self, 32768);
    if (!n || n >= 32768) return (int)fail(L"GetModuleFileName");
    const wchar_t *tool = wcsrchr(self, L'\\');
    tool = tool ? tool + 1 : self;
    if (_wcsicmp(tool, L"cl.exe") && _wcsicmp(tool, L"link.exe") && _wcsicmp(tool, L"lib.exe"))
        return ERROR_INVALID_PARAMETER;
    if (!load_environment()) return (int)fail(L"target environment");
    wchar_t binary[32768];
    if (swprintf_s(binary, 32768, L"%ls\\%ls", WRAPPER_TARGET_DIR, tool) < 0)
        return ERROR_FILENAME_EXCED_RANGE;
    /* Preserve the entire raw argument tail, including response-file quoting. */
    const wchar_t *tail = GetCommandLineW();
    int quoted = 0;
    while (*tail) {
        if (*tail == L'"') quoted = !quoted;
        else if (!quoted && (*tail == L' ' || *tail == L'\t')) break;
        ++tail;
    }
    size_t capacity = wcslen(binary) + wcslen(tail) + 3;
    if (capacity > 32767) return ERROR_FILENAME_EXCED_RANGE;
    wchar_t *command = calloc(capacity, sizeof(wchar_t));
    if (!command) return ERROR_NOT_ENOUGH_MEMORY;
    swprintf_s(command, capacity, L"\"%ls\"%ls", binary, tail);
    STARTUPINFOW start = {0};
    PROCESS_INFORMATION child = {0};
    start.cb = sizeof(start);
    start.dwFlags = STARTF_USESTDHANDLES;
    start.hStdInput = GetStdHandle(STD_INPUT_HANDLE);
    start.hStdOutput = GetStdHandle(STD_OUTPUT_HANDLE);
    start.hStdError = GetStdHandle(STD_ERROR_HANDLE);
    if (!CreateProcessW(binary, command, NULL, NULL, TRUE, 0, NULL, NULL, &start, &child)) {
        free(command);
        return (int)fail(L"CreateProcess");
    }
    free(command);
    CloseHandle(child.hThread);
    DWORD code = ERROR_GEN_FAILURE;
    if (WaitForSingleObject(child.hProcess, INFINITE) == WAIT_OBJECT_0)
        GetExitCodeProcess(child.hProcess, &code);
    CloseHandle(child.hProcess);
    return (int)code;
}

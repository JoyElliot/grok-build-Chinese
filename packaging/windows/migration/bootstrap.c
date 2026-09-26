/* GNU compatibility entry point for released executable-only updaters.
 * Generated bootstrap_payload.h binds the version and immutable migration pin.
 * The launcher remains stable; the MSVC runtime updates its own grok-zh.exe.
 */
#ifndef UNICODE
#define UNICODE
#endif
#ifndef _UNICODE
#define _UNICODE
#endif
#include <windows.h>
#include <bcrypt.h>
#include <stdio.h>
#include <stdlib.h>
#include <wchar.h>
#include <io.h>
#include <fcntl.h>
#include "bootstrap_payload.h"

#define CAP 32768
static BOOL WINAPI on_control(DWORD event) {
    /* The console also delivers these to the child. Keep the parent alive long
       enough to collect its exit code and clean up the owned temporary files. */
    return event == CTRL_C_EVENT || event == CTRL_BREAK_EVENT;
}

static int separator(wchar_t c) { return c == L'\\' || c == L'/'; }

static size_t path_root_length(const wchar_t *path) {
    size_t length = wcslen(path), start = 0;
    if (length >= 4 && !wcsncmp(path, L"\\\\?\\", 4)) {
        if (length >= 8 && !_wcsnicmp(path + 4, L"UNC\\", 4)) start = 8;
        else if (length >= 7 && path[5] == L':' && separator(path[6])) return 7;
        else return 0;
    } else if (length >= 2 && separator(path[0]) && separator(path[1])) {
        start = 2;
    } else {
        return length >= 3 && path[1] == L':' && separator(path[2]) ? 3 : 0;
    }
    /* UNC attributes must start at the complete server/share root, never at
       the server name alone. This also handles extended-length UNC paths. */
    size_t end = start;
    while (end < length && !separator(path[end])) ++end;
    if (end == start || end == length) return 0;
    start = ++end;
    while (end < length && !separator(path[end])) ++end;
    return end == start ? 0 : end;
}

static int plain_path(const wchar_t *path, int directory) {
    wchar_t copy[CAP];
    if (wcslen(path) >= CAP) return 0;
    size_t root = path_root_length(path);
    if (!root) return 0;
    wcscpy(copy, path);
    for (wchar_t *p = copy + root; *p; ++p) {
        if (*p != L'\\' && *p != L'/') continue;
        wchar_t saved = *p; *p = 0;
        DWORD attrs = GetFileAttributesW(copy);
        *p = saved;
        if (attrs == INVALID_FILE_ATTRIBUTES || (attrs & FILE_ATTRIBUTE_REPARSE_POINT)) return 0;
    }
    DWORD attrs = GetFileAttributesW(copy);
    return attrs != INVALID_FILE_ATTRIBUTES && !(attrs & FILE_ATTRIBUTE_REPARSE_POINT) &&
        (!!(attrs & FILE_ATTRIBUTE_DIRECTORY) == !!directory);
}

static int append(wchar_t *out, size_t *used, wchar_t c) {
    if (*used + 1 >= CAP) return 0;
    out[(*used)++] = c; out[*used] = 0; return 1;
}

/* Quote each argv item using the Windows CRT rules, including trailing '\\'. */
static int quote(wchar_t *out, size_t *used, const wchar_t *arg) {
    if (*used && !append(out, used, L' ')) return 0;
    if (!append(out, used, L'"')) return 0;
    size_t slashes = 0;
    for (;;) {
        wchar_t c = *arg++;
        if (c == L'\\') { ++slashes; continue; }
        size_t count = (c == L'"' || c == 0) ? slashes * 2 : slashes;
        for (size_t i = 0; i < count; ++i) if (!append(out, used, L'\\')) return 0;
        slashes = 0;
        if (!c) break;
        if (c == L'"' && !append(out, used, L'\\')) return 0;
        if (!append(out, used, c)) return 0;
    }
    return append(out, used, L'"');
}

static int run(const wchar_t *program, int argc, wchar_t **argv, DWORD *status, int diagnostics) {
    wchar_t command[CAP] = {0}; size_t used = 0;
    if (!quote(command, &used, program)) return 0;
    for (int i = 0; i < argc; ++i) if (!quote(command, &used, argv[i])) return 0;
    STARTUPINFOW startup = {0}; PROCESS_INFORMATION child = {0};
    startup.cb = sizeof(startup);
    if (diagnostics) {
        startup.dwFlags = STARTF_USESTDHANDLES;
        startup.hStdInput = GetStdHandle(STD_INPUT_HANDLE);
        startup.hStdOutput = GetStdHandle(STD_ERROR_HANDLE);
        startup.hStdError = GetStdHandle(STD_ERROR_HANDLE);
    }
    if (!CreateProcessW(program, command, NULL, NULL, TRUE, 0, NULL, NULL, &startup, &child)) return 0;
    CloseHandle(child.hThread);
    DWORD waited = WaitForSingleObject(child.hProcess, INFINITE);
    int ok = waited == WAIT_OBJECT_0 && GetExitCodeProcess(child.hProcess, status);
    CloseHandle(child.hProcess);
    return ok;
}

static int write_bytes(const wchar_t *path, const unsigned char *bytes, DWORD length) {
    HANDLE file = CreateFileW(path, GENERIC_WRITE, 0, NULL, CREATE_NEW, FILE_ATTRIBUTE_NORMAL, NULL);
    if (file == INVALID_HANDLE_VALUE) return 0;
    DWORD written = 0;
    int ok = WriteFile(file, bytes, length, &written, NULL) && written == length && FlushFileBuffers(file);
    CloseHandle(file); return ok;
}

static int json_path(FILE *file, const wchar_t *path) {
    int count = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, path, -1, NULL, 0, NULL, NULL);
    if (count <= 0) return 0;
    char *utf8 = malloc((size_t)count);
    if (!utf8) return 0;
    if (!WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, path, -1, utf8, count, NULL, NULL)) { free(utf8); return 0; }
    fputc('"', file);
    for (unsigned char *p = (unsigned char *)utf8; *p; ++p) {
        if (*p == '"' || *p == '\\') fputc('\\', file);
        if (*p < 32) fprintf(file, "\\u%04x", *p); else fputc(*p, file);
    }
    fputc('"', file); free(utf8); return !ferror(file);
}

static int ready(const wchar_t *root, const wchar_t *runtime, const wchar_t *marker) {
    wchar_t path[CAP];
    if (swprintf(path, CAP, L"%ls\\.grok-zh-msvc\\.grok-zh-bootstrap-ready", root) < 0 ||
        !plain_path(path, 0) || !plain_path(runtime, 0) || !plain_path(marker, 0)) return 0;
    FILE *file = _wfopen(path, L"rb");
    if (!file) return 0;
    const char expected[] = BOOTSTRAP_VERSION "\nwindows-x64-gnu-to-msvc-v1\n";
    char value[sizeof(expected)];
    size_t length = fread(value, 1, sizeof(value), file);
    int ok = length == sizeof(expected) - 1 && !memcmp(value, expected, length) && !ferror(file);
    fclose(file); return ok;
}

int wmain(int argc, wchar_t **argv) {
    wchar_t self[CAP], root[CAP], runtime[CAP], marker[CAP];
    DWORD length = GetModuleFileNameW(NULL, self, CAP);
    if (!length || length >= CAP || !plain_path(self, 0)) return 1;
    wcscpy(root, self); wchar_t *leaf = wcsrchr(root, L'\\');
    if (!leaf) return 1;
    int canonical = _wcsicmp(leaf + 1, L"grok-zh.exe") == 0; *leaf = 0;
    if (swprintf(runtime, CAP, L"%ls\\.grok-zh-msvc\\grok-zh.exe", root) < 0 ||
        swprintf(marker, CAP, L"%ls\\.grok-zh-msvc\\.grok-zh-install.json", root) < 0) return 1;
    SetConsoleCtrlHandler(on_control, TRUE);
    DWORD status = 1;
    if (argc == 2 && (!wcscmp(argv[1], L"--version") || !wcscmp(argv[1], L"-V"))) {
        /* Candidate names used by old updaters must report the new package's
           version, even when an older runtime exists next to the candidate. */
        if (canonical && ready(root, runtime, marker)) {
            return run(runtime, argc - 1, argv + 1, &status, 0) ? (int)status : 1;
        }
        printf("grok-zh %s (GNU migration launcher)\n", BOOTSTRAP_VERSION);
        return 0;
    }
    if (!canonical) {
        fputs("The migration launcher must be named grok-zh.exe.\n", stderr); return 1;
    }
    /* Local state was committed by the verified migration transaction. Future
       MSVC updates own this runtime, so do not require TEMP, PowerShell or any
       network access on the normal path. */
    if (ready(root, runtime, marker)) return run(runtime, argc - 1, argv + 1, &status, 0) ? (int)status : 1;

    wchar_t temp[CAP], work[CAP], script[CAP], online[CAP], context[CAP], powershell[CAP];
    DWORD tempLength = GetTempPathW(CAP, temp);
    if (!tempLength || tempLength + 100 >= CAP || !plain_path(temp, 1)) return 1;
    unsigned char random[16];
    if (BCryptGenRandom(NULL, random, sizeof(random), BCRYPT_USE_SYSTEM_PREFERRED_RNG) != 0) return 1;
    wchar_t token[33];
    for (int i = 0; i < 16; ++i) swprintf(token + i * 2, 3, L"%02x", random[i]);
    if (swprintf(work, CAP, L"%lsgrok-zh-migration-%ls", temp, token) < 0 || !CreateDirectoryW(work, NULL)) return 1;
    swprintf(script, CAP, L"%ls\\Invoke-Bootstrap.ps1", work);
    swprintf(online, CAP, L"%ls\\Install-GrokZhOnline.ps1", work);
    swprintf(context, CAP, L"%ls\\context.json", work);
    int ok = write_bytes(script, bootstrap_script, sizeof(bootstrap_script)) &&
        write_bytes(online, online_script, sizeof(online_script));
    if (ok) {
        HANDLE handle = CreateFileW(context, GENERIC_WRITE, 0, NULL, CREATE_NEW, FILE_ATTRIBUTE_NORMAL, NULL);
        int fd = handle == INVALID_HANDLE_VALUE ? -1 : _open_osfhandle((intptr_t)handle, _O_BINARY | _O_WRONLY);
        FILE *file = fd == -1 ? NULL : _fdopen(fd, "wb");
        if (!file && fd != -1) _close(fd);
        if (fd == -1 && handle != INVALID_HANDLE_VALUE) CloseHandle(handle);
        if (!file) ok = 0;
        else {
            fprintf(file, "{\"version\":\"%s\",\"migration\":%s,\"launcher\":", BOOTSTRAP_VERSION, MIGRATION_PIN);
            ok = json_path(file, self);
            if (fputs("}", file) == EOF) ok = 0;
            if (fclose(file) != 0) ok = 0;
        }
    }
    if (ok) {
        DWORD systemLength = GetSystemDirectoryW(powershell, CAP);
        if (!systemLength || systemLength + 60 >= CAP) ok = 0;
        else wcscat(powershell, L"\\WindowsPowerShell\\v1.0\\powershell.exe");
    }
    if (ok) {
        wchar_t *args[] = {L"-NoLogo", L"-NoProfile", L"-ExecutionPolicy", L"Bypass", L"-File", script, L"-ContextPath", context};
        ok = run(powershell, 8, args, &status, 1) && status == 0;
    }
    /* The loader removes its validated owned tree, including download files.
       These calls also handle failures before the loader could start. */
    DeleteFileW(context); DeleteFileW(script); DeleteFileW(online); RemoveDirectoryW(work);
    if (!ok || !plain_path(runtime, 0) || !plain_path(marker, 0)) {
        fputs("Migration did not finish. The GNU launcher is retained; retry or use the online installer.\n", stderr);
        return 1;
    }
    return run(runtime, argc - 1, argv + 1, &status, 0) ? (int)status : 1;
}

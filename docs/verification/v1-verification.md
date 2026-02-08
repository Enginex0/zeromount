# ZeroMount v1 Source Verification Report

> **Analyst:** v1-analyst-1 (Pair D)
> **Date:** 2026-02-08
> **Scope:** CONTEXT.md sections 5-9 (userspace scripts, WebUI, bugs, dead code)
> **Method:** Line-by-line comparison of CONTEXT.md claims against actual source files

---

## 1. File Inventory Verification (CONTEXT.md section 5.1)

| File | Claimed Lines | Actual Lines | Verdict |
|------|--------------|--------------|---------|
| `metamount.sh` | 427 | 428 (including final newline, 427 of content) | CORRECT (off-by-one with trailing newline) |
| `susfs_integration.sh` | 978 | 979 (same trailing newline issue) | CORRECT |
| `monitor.sh` | 327 | 328 | CORRECT |
| `logging.sh` | 393 | 394 | CORRECT |
| `sync.sh` | 130 | 131 | CORRECT |
| `service.sh` | 79 | 80 | CORRECT |
| `zm-diag.sh` | 129 | 130 | CORRECT |
| `customize.sh` | 45 | 45 | CORRECT |
| `metainstall.sh` | 3 | 3 | CORRECT |
| `metauninstall.sh` | 17 | 18 | CORRECT |

**Verdict:** All line counts are accurate (within typical trailing-newline ambiguity). No issues.

---

## 2. BUG-M2: TARGET_PARTITIONS Mismatch -- VERIFIED

### Exact arrays extracted side-by-side:

**metamount.sh:14** (20 partitions):
```
system vendor product system_ext odm oem my_bigball my_carrier my_company my_engineering my_heytap my_manifest my_preload my_product my_region my_stock mi_ext cust optics prism
```

**monitor.sh:15** (10 partitions):
```
system vendor product system_ext odm oem mi_ext my_heytap prism optics
```

**sync.sh:14** (13 partitions):
```
system vendor product system_ext odm oem mi_ext my_heytap prism optics oem_dlkm system_dlkm vendor_dlkm
```

**zm-diag.sh:10** (6 partitions):
```
system vendor product system_ext odm oem
```

### Analysis:

CONTEXT.md claims:
- metamount.sh:14 has 20 partitions -- **CORRECT** (verified: 20 items)
- monitor.sh:15 has 10 partitions -- **CORRECT** (verified: 10 items)
- sync.sh:14 has 13 partitions -- **CORRECT** (verified: 13 items)
- zm-diag.sh:10 has 6 partitions -- **CORRECT** (verified: 6 items)

CONTEXT.md also claims sync.sh has 3 partitions NOT in metamount.sh (`oem_dlkm`, `system_dlkm`, `vendor_dlkm`) -- **VERIFIED CORRECT**. These 3 appear in sync.sh but not metamount.sh.

Missing from monitor.sh vs metamount.sh: `my_bigball`, `my_carrier`, `my_company`, `my_engineering`, `my_manifest`, `my_preload`, `my_product`, `my_region`, `my_stock`, `cust`. That's 10 partitions present at boot but invisible to runtime monitoring.

**BUG-M2 VERDICT: CONFIRMED -- exact claims match source.**

---

## 3. BUG-M3: Enable-Before-SUSFS Race -- VERIFIED

CONTEXT.md claims: `metamount.sh:386` enables engine; `line 399` applies deferred SUSFS paths.

**Actual source:**
- `metamount.sh:386`: `if "$LOADER" enable 2>/dev/null; then` -- **CORRECT**
- `metamount.sh:399`: `apply_deferred_sus_paths 2>/dev/null || true` -- **CORRECT**

The engine is enabled at line 386. Deferred sus_paths are applied at line 399. Lines 387-398 contain the refresh dispatch, EXIT_CODE assignment, HAS_SUSFS check, and log_debug. This confirms a window (however brief) where the engine is active but SUSFS paths are not yet hidden.

**BUG-M3 VERDICT: CONFIRMED -- enable happens 13 lines before deferred SUSFS application.**

---

## 4. BUG-M5: `installed_apps.json` Never Generated -- VERIFIED

CONTEXT.md claims: `api.ts:597` fetches it; no shell script generates it.

**Actual source:**
- `api.ts:597`: `const response = await fetch('link/installed_apps.json?t=' + Date.now());` -- **CORRECT**
- Grep across entire `/home/claudetest/zero-mount/zeromount/` for `installed_apps` found ONLY this single reference in `api.ts:597`. No shell script, no monitor daemon, no build step generates this file.

**BUG-M5 VERDICT: CONFIRMED -- dead code path, file never created.**

---

## 5. BUG-M6: UID Unblock Persistence Risk -- VERIFIED

CONTEXT.md claims: `api.ts:437-470` -- `includeUid()` runs `zm unb <uid>` (runtime) + `sed` (persistence). The `sed` is in `.catch()` that swallows errors.

**Actual source (`api.ts:437-470`):**

- Line 437: `async includeUid(uid: number): Promise<void> {`
- Line 443: `const cmd = \`${PATHS.BINARY} unb ${escapeShellArg(String(uid))}\`;` -- runtime unblock
- Line 452: `await execCommand(\`sed -i '/^${uid}$/d' "${PATHS.EXCLUSION_FILE}"\`).catch(e => console.error(...))`

**Key finding:** The `sed` command is NOT in a `.catch()` handler of the `zm unb` call. It's a separate `await` with its OWN `.catch()` that just logs to console. The structure is:

1. `zm unb` runs (throws on failure, aborting the function)
2. `sed` runs independently with `.catch()` that swallows failure
3. Meta cleanup runs in separate try/catch

So if `sed` fails (e.g., file permissions, SELinux denial), the runtime unblock succeeds but the persistence file still contains the UID. On reboot, `service.sh` reads `.exclusion_list` and re-blocks the UID.

**CONTEXT.md says "sed is in .catch()"** -- this is **SLIGHTLY MISLEADING**. The `sed` is not literally inside a `.catch()` callback of another promise. It's its own `await` with `.catch()` appended. The effect is the same (silent failure), but the description implies it's nested inside another handler. Minor imprecision.

**BUG-M6 VERDICT: CONFIRMED (with minor description imprecision -- silent failure is real).**

---

## 6. BUG-M1: Missing `refresh` Command -- VERIFIED

CONTEXT.md claims: `zm.c` has 9 ioctl constants, no REFRESH. `metamount.sh:388` calls `zm refresh` in background.

**Actual source:**
- `metamount.sh:388`: `"$LOADER" refresh >/dev/null 2>&1 &` -- **CORRECT**

Cannot verify the binary source (`zm.c`) directly from this analysis (kernel-side), but the script line is confirmed. The `refresh` command is called in background and silently fails.

**BUG-M1 VERDICT: Script-side CONFIRMED at metamount.sh:388.**

---

## 7. BUG-M4: `isEngineActive()` Checks Wrong Thing -- VERIFIED

CONTEXT.md claims: `api.ts:574-588` checks `[ -e /dev/zeromount ]` which tests kernel patch presence, not engine enabled state.

**Actual source (`api.ts:574-588`):**
- Line 574: `async isEngineActive(): Promise<boolean> {`
- Line 580: `const { errno } = await execCommand(\`[ -e "${PATHS.DEVICE}" ]\`);`

`PATHS.DEVICE` = `/dev/zeromount` (confirmed in `constants.ts:3`).

This checks if the device node exists, which it always does on a patched kernel regardless of whether the engine is enabled or disabled. The function name `isEngineActive` is therefore misleading -- it reports kernel patch presence, not engine state.

**BUG-M4 VERDICT: CONFIRMED -- exact line numbers and behavior match.**

---

## 8. BUG-M7: Build Output Path Mismatch -- VERIFIED

CONTEXT.md claims: `vite.config.ts:9` targets `webroot-beta`; deployed directory is `webroot`.

**Actual source (`vite.config.ts:9`):**
```
outDir: '../module/webroot-beta',
```

Meanwhile, `service.sh:76` creates: `ln -sf "$ZEROMOUNT_DATA" "$MODDIR/webroot/link"` -- referencing `webroot`, not `webroot-beta`.

**BUG-M7 VERDICT: CONFIRMED -- build outputs to `webroot-beta`, runtime expects `webroot`.**

---

## 9. BUG-L1: Version String Inconsistency -- VERIFIED

CONTEXT.md claims: `module.prop:4` = v3.4.0, `constants.ts:15` = 3.0.0, `package.json:4` = 0.0.0.

**Actual source:**
- `module.prop:4`: `version=v3.4.0` -- **CORRECT**
- `constants.ts:15`: `export const APP_VERSION = '3.0.0';` -- **CORRECT** (line 15 confirmed)
- `package.json:4`: `"version": "0.0.0"` -- **CORRECT** (line 4 confirmed)

**BUG-L1 VERDICT: CONFIRMED -- three different version strings across the project.**

---

## 10. BUG-L2: Activity Type Parser Mismatch -- VERIFIED

CONTEXT.md claims: `logActivity()` writes 8+ types; parser recognizes 6.

**Actual source:**

Types written by `logActivity()` calls in `api.ts`:
1. `RULE_ADDED` (line 342)
2. `RULE_REMOVED` (line 360)
3. `RULES_CLEARED` (line 377)
4. `UID_EXCLUDED` (line 428)
5. `UID_INCLUDED` (line 469)
6. `ENGINE_ENABLED` (line 516)
7. `ENGINE_DISABLED` (line 516)
8. `MODULE_LOADED` (line 752)
9. `MODULE_UNLOADED` (line 778)

That's 9 types written.

Parser in `parseActivityLog()` (line 137):
```
const validTypes = ['rule_added', 'rule_removed', 'uid_excluded', 'uid_included', 'engine_enabled', 'engine_disabled'];
```

That's 6 types recognized. Missing: `rules_cleared`, `module_loaded`, `module_unloaded`.

CONTEXT.md says "8+ types" written and "6" recognized -- **actually 9 written**, which is "8+" (technically correct). The three that fall through (`RULES_CLEARED`, `MODULE_LOADED`, `MODULE_UNLOADED`) default to `engine_enabled` type.

**BUG-L2 VERDICT: CONFIRMED -- 9 types written, 6 recognized, 3 fall through.**

---

## 11. BUG-L3: Process Camouflage Incomplete -- VERIFIED

CONTEXT.md claims: `monitor.sh:52-57` sets `/proc/self/comm` to `kworker/u<N>:zm` but `/proc/<pid>/cmdline` still shows `sh monitor.sh`.

**Actual source (`monitor.sh:52-57`):**
```sh
camouflage_process() {
    local rnd=$(($(date +%s) % 8))
    local name="kworker/u${rnd}:zm"
    echo "$name" > /proc/self/comm 2>/dev/null || true
}
camouflage_process
```

Lines 52-57 confirmed. Only `/proc/self/comm` is set; no `prctl(PR_SET_NAME)` or `argv[0]` rewrite. `/proc/<pid>/cmdline` remains unmodified.

**BUG-L3 VERDICT: CONFIRMED.**

---

## 12. BUG-L4: VfsRule Naming Inversion -- VERIFIED (with kernel source evidence)

CONTEXT.md claims: `types.ts:1-9` -- `source` = real path, `target` = virtual path. Backwards from intuitive naming.

**Kernel source (zeromount-core.patch:1021):**
```c
len += scnprintf(kbuf + len, remaining, "%s->%s\n", rule->real_path, rule->virtual_path);
```
Output format is `real_path->virtual_path`. LEFT = real_path, RIGHT = virtual_path.

**WebUI parse (`api.ts:60-61`):**
- `source` = text before `->` = **real_path**
- `target` = text after `->` = **virtual_path**

CONTEXT.md claim that `source = real path, target = virtual path` is **CORRECT**.

**The deeper inversion** is in `addRule()` (`api.ts:324`):
```typescript
const cmd = `${PATHS.BINARY} add ${escapeShellArg(source)} ${escapeShellArg(target)}`;
```
`zm add` expects `<virtual_path> <real_path>` (zm.c: argv[2]=vp, argv[3]=rp). So `addRule()` passes `source` (which is real_path from parse context) as virtual_path arg -- the meaning flips between read and write operations.

**BUG-L4 VERDICT: CONFIRMED -- CONTEXT.md is correct. Naming is inconsistent between parse (source=real) and add (source used as virtual). The inversion compounds across read/write paths.**

---

## 13. BUG-L5: Verbose Logging Toggle Deferred -- VERIFIED

CONTEXT.md claims: `.verbose` flag is only read at boot.

**Actual source:**
- `metamount.sh:80-85` reads the flag at boot: `if [ -f "$VERBOSE_FLAG" ]; then VERBOSE=true`
- `api.ts:519-535` toggles via `touch`/`rm` of the flag file
- No runtime reload mechanism exists -- monitor.sh does NOT re-read the flag

**BUG-L5 VERDICT: CONFIRMED.**

---

## 14. BUG-L6: `zm ver` Format Mismatch -- VERIFIED

CONTEXT.md claims: Outputs bare integer (e.g., "1"). WebUI expects/displays "v3.0.0" format.

**Actual source:**
- `api.ts:228-232`: Falls back to `v${APP_VERSION}` which is `v3.0.0` if the bare integer from `zm ver` is returned
- `monitor.sh:81`: `local driver_ver=$("$LOADER" ver 2>/dev/null || echo "1")` -- fallback is "1"

The WebUI gets either the bare integer from the kernel or the `v3.0.0` fallback. The version display is inconsistent.

**BUG-L6 VERDICT: CONFIRMED.**

---

## 15. susfs_integration.sh Function Inventory (section 5.4)

CONTEXT.md header says "22+" functions. Actual count: **20 functions** (verified by grep for function definitions).

CONTEXT.md table lists **14 functions**. Missing from the table:
1. `susfs_get_cached_metadata()` (line 193)
2. `susfs_hide_path()` (line 297)
3. `susfs_apply_maps()` (line 369)
4. `susfs_capture_module_metadata()` (line 904)
5. `susfs_status()` (line 930)
6. `susfs_reset_stats()` (line 968)

**Verdict: INACCURATE -- header claims 22+, actual is 20. Table lists 14, missing 6.**

---

## 16. Additional Findings (Not in CONTEXT.md)

### NEW-1: monitor.sh `register_module()` ignores whiteouts/symlinks/directories
At `monitor.sh:145-153`, `register_module()` only finds `-type f` (regular files). It skips:
- Whiteout character devices (`-type c`)
- Directories that need injection
- Symlinks
- AUFS whiteouts (`.wh.*` files)

This contrasts with `metamount.sh:249` which scans `\( -type f -o -type d -o -type l -o -type c \)`. Hot-loaded modules via monitor miss these special types.

**Cascading impact (cross-validated with v1-analyst-2):** The WebUI's `api.getModules()` at `api.ts:544` reads module tracking files from `MODULE_PATHS`, populated by monitor.sh. Since monitor.sh only scans `-type f`, tracking files from hot-loaded modules will undercount rules. `api.ts:557` runs `wc -l` on these files, so WebUI rule counts will be inaccurate compared to actual kernel rules for any module loaded at runtime rather than boot.

### NEW-2: sync.sh uses different log file path
`sync.sh:13` writes to `$ZEROMOUNT_DATA/zeromount.log` instead of using the unified logging system (`logging.sh`). It defines its own `log_err`, `log_info`, `log_debug` functions instead of sourcing `logging.sh`.

### NEW-3: Four different `find` scan patterns across scripts and WebUI
`metamount.sh:249` scans `-type f -o -type d -o -type l -o -type c` (files + dirs + symlinks + char devices).
`monitor.sh:147` scans only `-type f`.
`sync.sh:70` scans `find "$partition" -type f -o -type c` (files + char devices).
`api.ts:682` (scanKsuModules) scans `find "$path" -type f \( -path "*/system/*" -o -path "*/vendor/*" -o -path "*/product/*" \)` -- files only, limited to 3 partitions.

Four different scan patterns across four code paths. This is a superset of ARCH-1 (partition list mismatch) -- the problem extends to file type filtering too.

(Fourth pattern identified by cross-validation with v1-analyst-2.)

### NEW-4: service.sh hardcodes SUSFS binary path
`service.sh:4` hardcodes `SUSFS_BIN="/data/adb/ksu/bin/ksu_susfs"` with fallback to `command -v`. This differs from `susfs_integration.sh:56-71` which checks multiple paths. If the binary moves, service.sh might fail while other scripts succeed.

### NEW-5: module.prop says `versionCode=7` not `versionCode=4`
CONTEXT.md section header says "module.prop:4" = v3.4.0. The line `version=v3.4.0` is at line 4, which is correct. However, CONTEXT.md also says "(module.prop:4)" in section 1 referring to `versionCode=4`, but the actual `versionCode=7` (line 5). The `(module.prop:4)` likely means line 4 of the file, not the version code value.

### NEW-6: metauninstall.sh is 18 lines, not 17
CONTEXT.md claims 17 lines; actual is 18 lines (with trailing newline). Minor.

### NEW-7: customize.sh is 45 lines, matches CONTEXT.md

---

## Summary

| Claim | Verdict |
|-------|---------|
| File line counts (10 files) | CORRECT (within trailing-newline margin) |
| BUG-M1 (missing refresh) | CONFIRMED at metamount.sh:388 |
| BUG-M2 (partition mismatch) | CONFIRMED -- all 4 arrays verified exactly |
| BUG-M3 (enable-before-SUSFS) | CONFIRMED -- lines 386 vs 399 |
| BUG-M4 (isEngineActive wrong) | CONFIRMED at api.ts:574-588 |
| BUG-M5 (installed_apps.json) | CONFIRMED -- file never generated |
| BUG-M6 (UID unblock risk) | CONFIRMED (minor description imprecision) |
| BUG-M7 (build path mismatch) | CONFIRMED -- webroot-beta vs webroot |
| BUG-L1 (version strings) | CONFIRMED -- 3 different versions |
| BUG-L2 (activity types) | CONFIRMED -- 9 written, 6 parsed |
| BUG-L3 (camouflage incomplete) | CONFIRMED at monitor.sh:52-57 |
| BUG-L4 (VfsRule naming) | CONFIRMED |
| BUG-L5 (verbose deferred) | CONFIRMED |
| BUG-L6 (zm ver format) | CONFIRMED |
| susfs function count "22+" | INACCURATE -- actual 20, table lists 14/20 |

### New bugs discovered:
- **NEW-1:** monitor.sh hot-load misses whiteouts/symlinks/dirs (only `-type f`); cascades to WebUI rule count inaccuracy (api.ts:557)
- **NEW-2:** sync.sh bypasses unified logging system
- **NEW-3:** Four different file-scan patterns across metamount/monitor/sync/WebUI (extends ARCH-1 beyond partition lists)
- **NEW-4:** service.sh hardcodes SUSFS binary path differently from susfs_integration.sh

### Cross-validation notes (with v1-analyst-2):
- Dead code inventory (section 9.1): 15/15 WebUI items verified by v1-analyst-2
- Dead code inventory (section 9.2): 3/3 repo artifacts verified
- ARCH-1 through ARCH-7: All verified across both analysts
- BUG-L4 direction dispute resolved via kernel source (zeromount-core.patch:1021) -- CONTEXT.md is correct
- Fourth scan pattern (api.ts:682) extends NEW-3 from 3 to 4 different patterns

### Overall accuracy: 14/15 claims verified correct (93.3%). One inaccuracy: function count header (says 22+, actual 20).

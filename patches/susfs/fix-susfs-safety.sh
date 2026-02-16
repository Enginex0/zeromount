#!/bin/bash
# fix-susfs-safety.sh
# Applies security and correctness fixes to upstream SUSFS source:
# - strncpy null-termination (10+ locations)
# - Spin lock race conditions in kstat/open_redirect hash lookups
# - NULL deref in cmdline_or_bootconfig and enabled_features (deref after failed kzalloc)
# - Remove unused fsnotify_backend.h include
# - Change sus_mount default from false to true
#
# Usage: ./fix-susfs-safety.sh <SUSFS_KERNEL_PATCHES_DIR>

set -e

SUSFS_DIR="$1"

if [ -z "$SUSFS_DIR" ]; then
    echo "Usage: $0 <SUSFS_KERNEL_PATCHES_DIR>"
    exit 1
fi

SUSFS_C="$SUSFS_DIR/fs/susfs.c"

if [ ! -f "$SUSFS_C" ]; then
    echo "FATAL: missing $SUSFS_C"
    exit 1
fi

echo "=== fix-susfs-safety ==="
fix_count=0

# --- 1. (skipped: fsnotify_backend.h required for sdcard monitor) ---

# --- 2. Fix trailing whitespace in disabled log macros ---
if grep -q 'SUSFS_LOGI(fmt, \.\.\.) $' "$SUSFS_C"; then
    echo "[+] Fixing trailing whitespace in disabled log macros"
    sed -i 's/#define SUSFS_LOGI(fmt, \.\.\.) $/#define SUSFS_LOGI(fmt, ...)/' "$SUSFS_C"
    sed -i 's/#define SUSFS_LOGE(fmt, \.\.\.) $/#define SUSFS_LOGE(fmt, ...)/' "$SUSFS_C"
    ((fix_count++)) || true
else
    echo "[=] Log macros already clean"
fi

# --- 3. strncpy null-termination fixes ---
# After every strncpy, ensure the buffer is null-terminated.
# Pattern: strncpy(dst, src, SIZE - 1); -> add dst[SIZE-1] = '\0';
# We target specific anchor patterns rather than blind replacement.

echo "[+] Applying strncpy null-termination fixes"

# 3a. android_data_path.target_pathname
if ! grep -A1 'android_data_path.target_pathname' "$SUSFS_C" | grep -q '\[SUSFS_MAX_LEN_PATHNAME-1\].*\\0'; then
    sed -i '/strncpy(android_data_path.target_pathname, info.target_pathname, SUSFS_MAX_LEN_PATHNAME-1);/a \\t\tandroid_data_path.target_pathname[SUSFS_MAX_LEN_PATHNAME-1] = '"'"'\\0'"'"';' "$SUSFS_C"
    ((fix_count++)) || true
fi

# 3b. sdcard_path.target_pathname
if ! grep -A1 'sdcard_path.target_pathname' "$SUSFS_C" | grep -q '\[SUSFS_MAX_LEN_PATHNAME-1\].*\\0'; then
    sed -i '/strncpy(sdcard_path.target_pathname, info.target_pathname, SUSFS_MAX_LEN_PATHNAME-1);/a \\t\tsdcard_path.target_pathname[SUSFS_MAX_LEN_PATHNAME-1] = '"'"'\\0'"'"';' "$SUSFS_C"
    ((fix_count++)) || true
fi

# 3c-d. sus_path: new_list->info.target_pathname + new_list->target_pathname (3 code blocks)
# ADD blocks use 2-tab indent, sus_path_loop uses 1-tab — detect from strncpy line itself
# State-based: after seeing strncpy, insert null-term before the NEXT line (handles adjacent pairs)
awk '
{
    if (pending_field != "") {
        if ($0 !~ /target_pathname\[SUSFS_MAX_LEN_PATHNAME *- *1\]/) {
            print pending_indent "new_list->" pending_field "[SUSFS_MAX_LEN_PATHNAME-1] = '"'"'\\0'"'"';"
        }
        pending_field = ""
    }
    print
    if ($0 ~ /strncpy\(new_list->(info\.)?target_pathname,.*SUSFS_MAX_LEN_PATHNAME *- *1\);/) {
        match($0, /^[\t]+/)
        pending_indent = substr($0, RSTART, RLENGTH)
        pending_field = ($0 ~ /strncpy\(new_list->info\./) ? "info.target_pathname" : "target_pathname"
    }
}
END {
    if (pending_field != "") {
        print pending_indent "new_list->" pending_field "[SUSFS_MAX_LEN_PATHNAME-1] = '"'"'\\0'"'"';"
    }
}
' "$SUSFS_C" > "$SUSFS_C.tmp" && mv "$SUSFS_C.tmp" "$SUSFS_C"
((fix_count++)) || true

# 3e. uname release/version null-termination
if ! grep -A1 'strncpy(my_uname.release' "$SUSFS_C" | grep -q 'my_uname.release\[__NEW_UTS_LEN\]'; then
    # After the closing brace of the release if-else, add null term
    sed -i '/strncpy(my_uname.release, info.release, __NEW_UTS_LEN);/{
        n
        /}/a \\tmy_uname.release[__NEW_UTS_LEN] = '"'"'\\0'"'"';
    }' "$SUSFS_C"
    ((fix_count++)) || true
fi

if ! grep -A1 'strncpy(my_uname.version' "$SUSFS_C" | grep -q 'my_uname.version\[__NEW_UTS_LEN\]'; then
    sed -i '/strncpy(my_uname.version, info.version, __NEW_UTS_LEN);/{
        n
        /}/a \\tmy_uname.version[__NEW_UTS_LEN] = '"'"'\\0'"'"';
    }' "$SUSFS_C"
    ((fix_count++)) || true
fi

# 3f. spoof_uname tmp->release/version null-termination
if ! grep -A1 'strncpy(tmp->release' "$SUSFS_C" | grep -q 'tmp->release\[__NEW_UTS_LEN\]'; then
    sed -i '/strncpy(tmp->release, my_uname.release, __NEW_UTS_LEN);/a \\ttmp->release[__NEW_UTS_LEN] = '"'"'\\0'"'"';' "$SUSFS_C"
    ((fix_count++)) || true
fi

if ! grep -A1 'strncpy(tmp->version' "$SUSFS_C" | grep -q 'tmp->version\[__NEW_UTS_LEN\]'; then
    sed -i '/strncpy(tmp->version, my_uname.version, __NEW_UTS_LEN);/a \\ttmp->version[__NEW_UTS_LEN] = '"'"'\\0'"'"';' "$SUSFS_C"
    ((fix_count++)) || true
fi

# 3g. susfs_show_variant null-termination
if ! grep -A1 'strncpy(info.susfs_variant' "$SUSFS_C" | grep -q 'susfs_variant\[SUSFS_MAX_VARIANT_BUFSIZE-1\]'; then
    sed -i '/strncpy(info.susfs_variant, SUSFS_VARIANT, SUSFS_MAX_VARIANT_BUFSIZE-1);/a \\tinfo.susfs_variant[SUSFS_MAX_VARIANT_BUFSIZE-1] = '"'"'\\0'"'"';' "$SUSFS_C"
    ((fix_count++)) || true
fi

# 3h. susfs_show_version null-termination
if ! grep -A1 'strncpy(info.susfs_version' "$SUSFS_C" | grep -q 'susfs_version\[SUSFS_MAX_VERSION_BUFSIZE-1\]'; then
    sed -i '/strncpy(info.susfs_version, SUSFS_VERSION, SUSFS_MAX_VERSION_BUFSIZE-1);/a \\tinfo.susfs_version[SUSFS_MAX_VERSION_BUFSIZE-1] = '"'"'\\0'"'"';' "$SUSFS_C"
    ((fix_count++)) || true
fi

# 3i. open_redirect: new_entry->target_pathname and new_entry->redirected_pathname
# These appear in susfs_add_open_redirect() — the per-UID variant
# State-based: track pending null-term without consuming lines via getline
awk '
{
    if (pending_field != "") {
        if ($0 !~ /\[SUSFS_MAX_LEN_PATHNAME-1\]/) {
            print pending_indent "new_entry->" pending_field "[SUSFS_MAX_LEN_PATHNAME-1] = '"'"'\\0'"'"';"
        }
        pending_field = ""
    }
    print
    if ($0 ~ /strncpy\(new_entry->target_pathname, info\.target_pathname, SUSFS_MAX_LEN_PATHNAME-1\);/) {
        match($0, /^[\t]+/)
        pending_indent = substr($0, RSTART, RLENGTH)
        pending_field = "target_pathname"
    } else if ($0 ~ /strncpy\(new_entry->redirected_pathname, info\.redirected_pathname, SUSFS_MAX_LEN_PATHNAME-1\);/) {
        match($0, /^[\t]+/)
        pending_indent = substr($0, RSTART, RLENGTH)
        pending_field = "redirected_pathname"
    }
}
END {
    if (pending_field != "") {
        print pending_indent "new_entry->" pending_field "[SUSFS_MAX_LEN_PATHNAME-1] = '"'"'\\0'"'"';"
    }
}
' "$SUSFS_C" > "$SUSFS_C.tmp" && mv "$SUSFS_C.tmp" "$SUSFS_C"
((fix_count++)) || true

# --- 4. Spin lock race fixes in sus_kstat ---
# 4a. susfs_update_sus_kstat: full lock rewrite matching fork pattern
# Upstream holds no lock during hash iteration and does hash_del/kfree unlocked.
# Fork pattern: lock before loop, unlock after strcmp match (before sleeping ops),
# re-lock around hash_del+hash_add, kfree after unlock, unlock after loop end.
if ! grep -B2 'hash_for_each_safe(SUS_KSTAT_HLIST' "$SUSFS_C" | grep -q 'spin_lock.*susfs_spin_lock_sus_kstat'; then
    echo "[+] Fixing spin lock ordering in susfs_update_sus_kstat"
    awk '
    /^void susfs_update_sus_kstat\(/ { in_func = 1 }
    in_func && /hash_for_each_safe\(SUS_KSTAT_HLIST/ {
        print "\tspin_lock(&susfs_spin_lock_sus_kstat);"
        print
        next
    }
    in_func && /if [(]!strcmp[(]tmp_entry->info[.]target_pathname, info[.]target_pathname[)][)] [{]/ {
        print
        print "\t\t\tspin_unlock(&susfs_spin_lock_sus_kstat);"
        next
    }
    in_func && /hash_del\(&tmp_entry->node\);/ {
        # upstream: hash_del + kfree + spin_lock + hash_add + spin_unlock
        # fork:     spin_lock + hash_del + hash_add + spin_unlock + kfree
        # consume the next 4 lines (kfree, spin_lock, hash_add, spin_unlock)
        getline kfree_line
        getline lock_line
        getline add_line
        getline unlock_line
        print "\t\t\tspin_lock(&susfs_spin_lock_sus_kstat);"
        print "\t\t\thash_del(&tmp_entry->node);"
        # extract hash_add line content (strip leading whitespace, reindent)
        gsub(/^[\t ]+/, "", add_line)
        print "\t\t\t" add_line
        print "\t\t\tspin_unlock(&susfs_spin_lock_sus_kstat);"
        # kfree after unlock
        gsub(/^[\t ]+/, "", kfree_line)
        print "\t\t\t" kfree_line
        next
    }
    in_func && /^out_copy_to_user:/ {
        print "\tspin_unlock(&susfs_spin_lock_sus_kstat);"
        print
        in_func = 0
        next
    }
    { print }
    ' "$SUSFS_C" > "$SUSFS_C.tmp" && mv "$SUSFS_C.tmp" "$SUSFS_C"
    ((fix_count++)) || true
fi

# 4b. susfs_sus_ino_for_generic_fillattr: wrap with irqsave
if ! grep -B2 'hash_for_each_possible(SUS_KSTAT_HLIST, entry, node, ino)' "$SUSFS_C" | head -1 | grep -q 'spin_lock_irqsave'; then
    echo "[+] Fixing spin lock race in susfs_sus_ino_for_generic_fillattr"
    # Add unsigned long flags; variable and spin_lock_irqsave before hash lookup
    sed -i '/^void susfs_sus_ino_for_generic_fillattr/,/^}/ {
        /struct st_susfs_sus_kstat_hlist \*entry;/a \\tunsigned long flags;\n\n\tspin_lock_irqsave(\&susfs_spin_lock_sus_kstat, flags);
        /return;/i \\t\t\tspin_unlock_irqrestore(\&susfs_spin_lock_sus_kstat, flags);
    }' "$SUSFS_C"
    # Add unlock after the loop
    sed -i '/^void susfs_sus_ino_for_generic_fillattr/,/^}/ {
        /^}/ i\\tspin_unlock_irqrestore(\&susfs_spin_lock_sus_kstat, flags);
    }' "$SUSFS_C"
    ((fix_count++)) || true
fi

# 4c. susfs_sus_ino_for_show_map_vma: same pattern
if ! grep -A3 'void susfs_sus_ino_for_show_map_vma' "$SUSFS_C" | grep -q 'unsigned long flags'; then
    echo "[+] Fixing spin lock race in susfs_sus_ino_for_show_map_vma"
    sed -i '/^void susfs_sus_ino_for_show_map_vma/,/^}/ {
        /struct st_susfs_sus_kstat_hlist \*entry;/a \\tunsigned long flags;\n\n\tspin_lock_irqsave(\&susfs_spin_lock_sus_kstat, flags);
        /return;/i \\t\t\tspin_unlock_irqrestore(\&susfs_spin_lock_sus_kstat, flags);
    }' "$SUSFS_C"
    sed -i '/^void susfs_sus_ino_for_show_map_vma/,/^}/ {
        /^}/ i\\tspin_unlock_irqrestore(\&susfs_spin_lock_sus_kstat, flags);
    }' "$SUSFS_C"
    ((fix_count++)) || true
fi

# 4d. susfs_get_redirected_path: rewrite with result variable + spin lock
if ! grep -A3 'susfs_get_redirected_path(unsigned long ino)' "$SUSFS_C" | grep -q 'result.*ERR_PTR'; then
    echo "[+] Fixing spin lock race in susfs_get_redirected_path"
    awk '
    /^struct filename\* susfs_get_redirected_path\(unsigned long ino\)/ {
        print
        in_func = 1
        next
    }
    in_func && /struct st_susfs_open_redirect_hlist \*entry;/ {
        print
        print "\tstruct filename *result = ERR_PTR(-ENOENT);"
        print ""
        print "\tspin_lock(&susfs_spin_lock_open_redirect);"
        next
    }
    in_func && /return getname_kernel\(entry->redirected_pathname\);/ {
        print "\t\t\tresult = getname_kernel(entry->redirected_pathname);"
        print "\t\t\tbreak;"
        next
    }
    in_func && /return ERR_PTR\(-ENOENT\);/ {
        print "\tspin_unlock(&susfs_spin_lock_open_redirect);"
        print "\treturn result;"
        in_func = 0
        next
    }
    { print }
    ' "$SUSFS_C" > "$SUSFS_C.tmp" && mv "$SUSFS_C.tmp" "$SUSFS_C"
    ((fix_count++)) || true
fi

# --- 5. NULL deref fixes ---
# 5a/5b: kzalloc returns NULL → code dereferences info->err.
# Only replace the block inside if (!info) { ... }, not subsequent error paths.
# Pattern: if (!info) { info->err = -ENOMEM; goto out_copy_to_user; }
# We match the 3-line sequence and replace with SUSFS_LOGE + return.
if grep -q 'if (!info)' "$SUSFS_C" && grep -A1 'if (!info)' "$SUSFS_C" | grep -q 'info->err = -ENOMEM'; then
    echo "[+] Fixing NULL deref in kzalloc error paths"
    awk '
    /if \(!info\) \{/ {
        if (getline l1 > 0 && l1 ~ /info->err = -ENOMEM/) {
            if (getline l2 > 0 && l2 ~ /goto out_copy_to_user/) {
                if (getline l3 > 0 && l3 ~ /\}/) {
                    print "\tif (!info) {"
                    print "\t\tSUSFS_LOGE(\"Failed to allocate memory\\n\");"
                    print "\t\treturn;"
                    print "\t}"
                    next
                }
            }
        }
        print
        if (l1 != "") print l1
        if (l2 != "") print l2
        if (l3 != "") print l3
        next
    }
    { print }
    ' "$SUSFS_C" > "$SUSFS_C.tmp" && mv "$SUSFS_C.tmp" "$SUSFS_C"
    ((fix_count++)) || true
else
    echo "[=] kzalloc NULL deref already fixed"
fi

# --- 6. Change sus_mount default: false -> true ---
if grep -q 'susfs_hide_sus_mnts_for_non_su_procs = false' "$SUSFS_C"; then
    echo "[+] Changing sus_mount default to true"
    sed -i 's/bool susfs_hide_sus_mnts_for_non_su_procs = false;/bool susfs_hide_sus_mnts_for_non_su_procs = true;/' "$SUSFS_C"
    ((fix_count++)) || true
else
    echo "[=] sus_mount default already true"
fi

# --- 7. (removed: trailing whitespace fix was a no-op) ---

# --- 8. Fix format specifier: spoofed_size is loff_t (long long), not unsigned int ---
# Upstream uses '%u' for spoofed_size in the #else (non-STAT64) SUSFS_LOGI paths
if grep -q "spoofed_size: '%u'" "$SUSFS_C"; then
    echo "[+] Fixing spoofed_size format specifier (%u -> %llu)"
    sed -i "s/spoofed_size: '%u'/spoofed_size: '%llu'/g" "$SUSFS_C"
    ((fix_count++)) || true
else
    echo "[=] spoofed_size format specifier already correct"
fi

# --- 10. Remove EACCES permission leak from SUS_PATH in GKI patch ---
# Older upstream versions return ERR_PTR(-EACCES) on create/excl lookups,
# which leaks SUSFS presence to detector apps. Replace with blank lines
# to preserve patch hunk line counts.
for patch_file in "$SUSFS_DIR"/50_add_susfs_in_gki-*.patch; do
    [ -f "$patch_file" ] || continue
    if grep -q 'ERR_PTR(-EACCES)' "$patch_file"; then
        echo "[+] Removing EACCES permission leak from $(basename "$patch_file")"
        awk '
        # 5.10: if (flags & (LOOKUP_CREATE | LOOKUP_EXCL)) { return ERR_PTR(-EACCES); }
        /^\+[[:space:]]*if \(flags & \(LOOKUP_CREATE \| LOOKUP_EXCL\)\) \{/ {
            print "+"; getline; print "+"; getline; print "+"
            next
        }
        # 6.6: if (create_flags) { dentry = ERR_PTR(-EACCES); goto unlock; }
        /^\+[[:space:]]*if \(create_flags\) \{/ {
            saved = $0
            if (getline > 0 && $0 ~ /ERR_PTR\(-EACCES\)/) {
                print "+"; print "+"
                getline; print "+"; getline; print "+"
                next
            }
            print saved
        }
        { print }
        ' "$patch_file" > "$patch_file.tmp" && mv "$patch_file.tmp" "$patch_file"
        ((fix_count++)) || true
    else
        echo "[=] No EACCES permission leak in $(basename "$patch_file")"
    fi
done

echo "=== Done: $fix_count fixes applied ==="

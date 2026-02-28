#!/system/bin/sh
# Run: adb shell su -c 'sh /data/local/tmp/test_adb_hide.sh'
# Validates USB debug hiding at root context. For UID>=10000 vectors, use the APK.

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'
PASS=0; FAIL=0; WARN=0

pass() { printf "${GREEN}PASS${NC} %s\n" "$1"; PASS=$((PASS+1)); }
fail() { printf "${RED}FAIL${NC} %s\n" "$1"; FAIL=$((FAIL+1)); }
warn() { printf "${YELLOW}WARN${NC} %s\n" "$1"; WARN=$((WARN+1)); }

echo "=== PHASE 0: BLOCKING QUESTIONS ==="

# Q1: props spoofed before zygote starts
ZM=$(dmesg 2>/dev/null | grep "zeromount.*USB debug props spoofed early" | head -1 | grep -oE '\[ *[0-9]+' | tr -d '[ ')
ZY=$(dmesg 2>/dev/null | grep "init: starting service 'zygote'" | head -1 | grep -oE '\[ *[0-9]+' | tr -d '[ ')
if [ -n "$ZM" ] && [ -n "$ZY" ] && [ "$ZM" -lt "$ZY" ]; then
    pass "Q1: USB props spoofed (${ZM}s) before zygote (${ZY}s)"
else
    warn "Q1: timing unverified (zm_ts=${ZM:-missing} zy_ts=${ZY:-missing})"
fi

# Q4: adbd PID stability through stop/start cycle
PID_BEFORE=$(ps -A 2>/dev/null | awk '$NF=="adbd"{print $1}' | head -1)
stop adbd 2>/dev/null; sleep 1
PID_STOPPED=$(ps -A 2>/dev/null | awk '$NF=="adbd"{print $1}' | head -1)
start adbd 2>/dev/null; sleep 2
PID_AFTER=$(ps -A 2>/dev/null | awk '$NF=="adbd"{print $1}' | head -1)
warn "Q4: adbd pid: before=${PID_BEFORE:-none} stopped=${PID_STOPPED:-none} after=${PID_AFTER:-none}"

# Q5: USB sysfs paths
for P_PATH in /sys/class/android_usb/android0/state /sys/class/android_usb/android0/functions; do
    if [ -e "$P_PATH" ]; then
        pass "Q5: $P_PATH = $(cat "$P_PATH" 2>/dev/null)"
    else
        warn "Q5: $P_PATH not found"
    fi
done

# Q6: adbd stays stopped after explicit stop (init doesn't auto-restart)
stop adbd 2>/dev/null; sleep 3
ADBD_SVC=$(getprop init.svc.adbd)
if [ "$ADBD_SVC" = "stopped" ]; then
    pass "Q6: adbd stays stopped"
else
    fail "Q6: adbd restarted (init.svc.adbd=$ADBD_SVC)"
fi
start adbd 2>/dev/null

echo ""
echo "=== BASELINE (root — real values) ==="
printf "  adb_enabled           = %s\n" "$(settings get global adb_enabled 2>/dev/null)"
printf "  persist.sys.usb.config= %s\n" "$(getprop persist.sys.usb.config)"
printf "  sys.usb.config        = %s\n" "$(getprop sys.usb.config)"
printf "  init.svc.adbd         = %s\n" "$(getprop init.svc.adbd)"
printf "  /proc/net/tcp :5555   = %s\n" "$(grep ' 15B3' /proc/net/tcp 2>/dev/null | head -1 || echo 'NOT_FOUND')"
printf "  adbd proc             = %s\n" "$(ps -A 2>/dev/null | grep ' adbd$' | head -1 || echo 'NOT_FOUND')"

echo ""
echo "=== KERNEL FILTER (shell UID 2000 — whitelisted, should see real values) ==="

TCP_SHELL=$(su shell -c 'grep " 15B3" /proc/net/tcp 2>/dev/null' 2>/dev/null)
if [ -n "$TCP_SHELL" ]; then
    pass "V10: shell sees ADB port (whitelisted UID 2000)"
else
    warn "V10: shell cannot see ADB port — ADB may not be on port 5555"
fi

TCP6_SHELL=$(su shell -c 'grep " 15B3" /proc/net/tcp6 2>/dev/null' 2>/dev/null)
if [ -n "$TCP6_SHELL" ]; then
    pass "V10b: shell sees ADB tcp6 port (whitelisted UID 2000)"
else
    warn "V10b: shell cannot see ADB tcp6 port — may not be listening on tcp6"
fi

UNIX_SHELL=$(su shell -c 'grep -E "adbd|jdwp" /proc/net/unix 2>/dev/null' 2>/dev/null)
if [ -n "$UNIX_SHELL" ]; then
    pass "V11: shell sees adbd/jdwp unix sockets (whitelisted UID 2000)"
else
    warn "V11: shell cannot see adbd/jdwp sockets — may not be running"
fi

echo ""
echo "=== V12: /proc hidepid (adbd process enumeration) ==="

HIDEPID=$(cat /proc/self/mountinfo 2>/dev/null | grep ' /proc ' | grep -oE 'hidepid=[^ ,]+' | head -1)
if [ -n "$HIDEPID" ]; then
    pass "V12: /proc mounted with $HIDEPID — untrusted_app cannot enumerate adbd"
else
    warn "V12: hidepid not detected in /proc mount — untrusted_app may see adbd via /proc/*/cmdline"
fi

echo ""
echo "=== KERNEL FILTER (untrusted_app UID 10000+ — should be hidden) ==="
echo "NOTE: Definitive untrusted_app tests require the APK (test/adbdetect/)."

echo ""
echo "=== PROPERTY SPOOF ==="
ZM_BIN=""
for b in /data/adb/modules/meta-zeromount/bin/arm64-v8a/zeromount \
         /data/adb/zeromount/bin/zeromount \
         zeromount; do
    if command -v "$b" >/dev/null 2>&1 || [ -x "$b" ]; then
        ZM_BIN="$b"; break
    fi
done

if [ -n "$ZM_BIN" ]; then
    HIDE=$("$ZM_BIN" config get adb.hide_usb_debugging 2>/dev/null)
else
    HIDE=$(cat /data/adb/zeromount/flags/hide_usb_debugging 2>/dev/null | head -1)
    [ "$HIDE" = "1" ] && HIDE="true"
fi

if [ "$HIDE" = "true" ]; then
    pass "CONFIG: adb.hide_usb_debugging=true"
    PERSIST=$(getprop persist.sys.usb.config)
    SYSCFG=$(getprop sys.usb.config)
    ADBD=$(getprop init.svc.adbd)
    case "$PERSIST" in
        mtp|charging|none) pass "V4/V7: persist.sys.usb.config=$PERSIST (no adb)" ;;
        *adb*) fail "V4/V7: persist.sys.usb.config=$PERSIST (contains adb)" ;;
        *) warn "V4/V7: persist.sys.usb.config=$PERSIST (unexpected)" ;;
    esac
    case "$SYSCFG" in
        mtp|charging|none) pass "V5/V8: sys.usb.config=$SYSCFG (no adb)" ;;
        *adb*) fail "V5/V8: sys.usb.config=$SYSCFG (contains adb)" ;;
        *) warn "V5/V8: sys.usb.config=$SYSCFG (unexpected)" ;;
    esac
    [ "$ADBD" = "stopped" ] && \
        pass "V6/V9: init.svc.adbd=stopped" || \
        fail "V6/V9: init.svc.adbd=$ADBD (expected stopped)"
else
    warn "adb.hide_usb_debugging not enabled — skipping prop spoof checks"
fi

echo ""
echo "=== REGRESSION (ADB connection must survive) ==="
echo "test_content" > /data/local/tmp/.adb_reg_test 2>/dev/null && \
    pass "REG: adb shell writes to /data/local/tmp work" && \
    rm -f /data/local/tmp/.adb_reg_test || \
    fail "REG: cannot write to /data/local/tmp"

ADBD_COUNT=$(ps -A 2>/dev/null | grep -c ' adbd$' 2>/dev/null || echo "0")
[ "$ADBD_COUNT" -gt "0" ] && \
    pass "REG: real adbd process running (ADB connection active)" || \
    warn "REG: adbd not in ps — connection may drop after test"

echo ""
echo "========================================"
printf "RESULTS: %d pass, %d fail, %d warn\n" "$PASS" "$FAIL" "$WARN"
echo "NOTE: V1-V14 definitive check requires com.test.adbdetect APK (runs as untrusted_app)"
echo "NOTE: Install + launch: adb install -r /tmp/adbdetect_build/apk/adbdetect_signed.apk"
echo "      adb shell am start -n com.test.adbdetect/.DetectorCheck"
echo "      adb logcat -s ADBDetect"
echo "========================================"
[ "$FAIL" -eq 0 ] && printf "${GREEN}ALL CHECKS PASSED${NC}\n" && exit 0 || \
    printf "${RED}%d FAILED${NC}\n" "$FAIL" && exit 1

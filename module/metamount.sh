#!/system/bin/sh
# KSU/APatch metamodule mount hook (post-fs-data phase).
# Pipeline runs at late_start (service.sh); this just unblocks KSU's gate.
MODDIR="${0%/*}"

ksud kernel notify-module-mounted 2>/dev/null
exit 0

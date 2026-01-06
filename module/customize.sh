#!/system/bin/sh
# Welcome Message
ui_print " "
ui_print "======================================="
ui_print "               NoMount                 "
ui_print "  Native Kernel Injection Metamodule   "
ui_print "======================================="
ui_print " "

ARCH=$(busybox uname -m)
case "$ARCH" in
	aarch64 | arm64 )
		ELF_BINARY="nm-arm64"
		;;
	armv7l | armv8l )
		ELF_BINARY="nm-arm"
		;;
	*)
		abort "[!] $ARCH not supported!"
		;;
esac

mv "$MODPATH/$ELF_BINARY" "$MODPATH/bin/nm"
rm -rf "$MODPATH/nm*"

ui_print "- Extracting module files..."
ui_print "- Setting permissions..."

chmod 755 "$MODPATH/bien/nm_loader" || abort "! Failed to set permissions"

ui_print "- Checking Kernel support..."

if [ -e "/dev/nomount" ]; then
  ui_print "  [OK] Driver /dev/nomount detected."
  ui_print "  [OK] System is ready for injection."
else
  ui_print " "
  ui_print "***************************************************"
  ui_print "* [!] WARNING: KERNEL DRIVER NOT DETECTED         *"
  ui_print "***************************************************"
  ui_print "* The device node /dev/nomount is missing.        *"
  ui_print "* *"
  ui_print "* This module will NOT FUNCTION until you flash   *"
  ui_print "* a Kernel compiled with CONFIG_NOMOUNT=y         *"
  ui_print "***************************************************"
  ui_print " "
  
  touch "$MODPATH/disable"
fi

if [ -f "/data/adb/nomount.log" ]; then
    rm -f "/data/adb/nomount.log"
fi

ui_print "- Installation complete."

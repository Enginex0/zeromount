ui_print " "
ui_print "======================================="
ui_print "              ZeroMount                "
ui_print "  Native Kernel Injection Metamodule   "
ui_print "======================================="
ui_print " "

ui_print "- Device Architecture: $ARCH"
if [ ! -f "$MODPATH/zm-$ARCH" ]; then
  abort "! Unsupported architecture: $ARCH"
fi
mkdir -p "$MODPATH/bin"
cp -f "$MODPATH/zm-$ARCH" "$MODPATH/bin/zm"
set_perm "$MODPATH/bin/zm" 0 0 0755
set_perm "$MODPATH/bin/arm64-v8a/aapt" 0 0 0755
set_perm "$MODPATH/bin/armeabi-v7a/aapt" 0 0 0755
set_perm "$MODPATH/service.sh" 0 0 0755
set_perm "$MODPATH/monitor.sh" 0 0 0755
rm -rf $MODPATH/zm*

ui_print "- Checking Kernel support..."
if [ -e "/dev/zeromount" ]; then
  ui_print "  [OK] Driver /dev/zeromount detected."
  ui_print "  [OK] System is ready for injection."
else
  ui_print " "
  ui_print "***************************************************"
  ui_print "* [!] WARNING: KERNEL DRIVER NOT DETECTED         *"
  ui_print "***************************************************"
  ui_print "* The device node /dev/zeromount is missing.      *"
  ui_print "* *"
  ui_print "* This module will NOT FUNCTION until you flash   *"
  ui_print "* a Kernel compiled with CONFIG_ZEROMOUNT=y       *"
  ui_print "***************************************************"
  ui_print " "
  
  touch "$MODPATH/disable"
fi

if [ -f "/data/adb/zeromount/zeromount.log" ]; then
    rm -f "/data/adb/zeromount/zeromount.log"
fi

ui_print "- Installation complete."

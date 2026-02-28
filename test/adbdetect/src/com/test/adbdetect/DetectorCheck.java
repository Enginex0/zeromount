package com.test.adbdetect;

import android.app.Activity;
import android.content.ContentResolver;
import android.os.Bundle;
import android.os.SystemProperties;
import android.provider.Settings;
import android.util.Log;
import android.widget.ScrollView;
import android.widget.TextView;

public class DetectorCheck extends Activity {
    private static final String TAG = "ADBDetect";
    static { System.loadLibrary("adbdetect"); }

    public static native String nativeGetProperty(String key);
    public static native String readTcpEntry(int port);
    public static native String readTcp6Entry(int port);
    public static native String readUnixAdbd();
    public static native String findAdbdProc();
    public static native String statUsbState();
    public static native String statAdbKeys();

    private int passCount = 0, failCount = 0;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        StringBuilder out = new StringBuilder();
        ContentResolver cr = getContentResolver();

        String v1 = String.valueOf(Settings.Global.getInt(cr, "adb_enabled", -1));
        String v2 = String.valueOf(Settings.Global.getInt(cr, "development_settings_enabled", -1));
        String v3 = String.valueOf(Settings.Global.getInt(cr, "adb_wifi_enabled", -1));

        checkEquals(out, "V1  Settings.adb_enabled", v1, "0", "-1");
        checkEquals(out, "V2  Settings.dev_settings", v2, "0", "-1");
        checkEquals(out, "V3  Settings.adb_wifi", v3, "0", "-1");

        String v4 = SystemProperties.get("persist.sys.usb.config", "");
        String v5 = SystemProperties.get("sys.usb.config", "");
        String v6 = SystemProperties.get("init.svc.adbd", "");

        checkNoAdb(out, "V4  Java persist.sys.usb.config", v4);
        checkNoAdb(out, "V5  Java sys.usb.config", v5);
        checkEquals(out, "V6  Java init.svc.adbd", v6, "stopped", "");

        String v7 = nativeGetProperty("persist.sys.usb.config");
        String v8 = nativeGetProperty("sys.usb.config");
        String v9 = nativeGetProperty("init.svc.adbd");

        checkNoAdb(out, "V7  Native persist.sys.usb.config", v7);
        checkNoAdb(out, "V8  Native sys.usb.config", v8);
        checkEquals(out, "V9  Native init.svc.adbd", v9, "stopped", "");

        checkHidden(out, "V10 /proc/net/tcp :5555", readTcpEntry(5555));
        checkHidden(out, "V10b /proc/net/tcp6 :5555", readTcp6Entry(5555));
        checkHidden(out, "V11 /proc/net/unix adbd", readUnixAdbd());
        checkHidden(out, "V12 /proc/[pid] adbd", findAdbdProc());

        checkNoAdb(out, "V13 /sys USB state", statUsbState());

        checkHidden(out, "V14 /data/misc/adb/adb_keys", statAdbKeys());

        String summary = String.format("\n--- RESULT: %d PASS / %d FAIL ---", passCount, failCount);
        out.append(summary);
        Log.i(TAG, "=== ADB DETECT RESULTS ===");
        Log.i(TAG, summary.trim());

        TextView tv = new TextView(this);
        tv.setTextSize(11);
        tv.setText(out.toString());
        tv.setTypeface(android.graphics.Typeface.MONOSPACE);
        ScrollView sv = new ScrollView(this);
        sv.addView(tv);
        setContentView(sv);
    }

    private void checkEquals(StringBuilder sb, String vector, String value, String... hidden) {
        boolean ok = false;
        for (String h : hidden) {
            if (value.equals(h)) { ok = true; break; }
        }
        record(sb, vector, value, ok);
    }

    private void checkNoAdb(StringBuilder sb, String vector, String value) {
        record(sb, vector, value, !value.contains("adb"));
    }

    private void checkHidden(StringBuilder sb, String vector, String value) {
        record(sb, vector, value, value.startsWith("NOT_FOUND") || value.startsWith("ERROR:"));
    }

    private void record(StringBuilder sb, String vector, String value, boolean ok) {
        String verdict = ok ? "PASS" : "FAIL";
        if (ok) passCount++; else failCount++;
        String line = String.format("[%s] %-38s = [%s]\n", verdict, vector, value);
        sb.append(line);
        Log.i(TAG, line.trim());
    }
}

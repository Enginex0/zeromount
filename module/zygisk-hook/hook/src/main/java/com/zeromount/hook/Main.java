package com.zeromount.hook;

import android.util.Log;

import com.v7878.r8.annotations.DoNotObfuscate;
import com.v7878.r8.annotations.DoNotObfuscateType;
import com.v7878.r8.annotations.DoNotShrink;
import com.v7878.r8.annotations.DoNotShrinkType;
import com.v7878.zygisk.ZygoteLoader;

import java.io.File;
import java.io.FileOutputStream;

@DoNotObfuscateType
@DoNotShrinkType
public class Main {
    static final String TAG = "ZeroMount-Settings";

    @SuppressWarnings({"unused", "ConfusingMainMethod"})
    @DoNotShrink
    @DoNotObfuscate
    public static void main() {
        Log.i(TAG, "Injected into " + ZygoteLoader.getPackageName());
        String status;
        try {
            SettingsHook.install();
            status = "active";
            Log.i(TAG, "ContentProvider hook installed");
        } catch (Throwable t) {
            Log.e(TAG, "hook failed", t);
            status = "failed";
        }
        writeStatus(status);
    }

    private static void writeStatus(String status) {
        File f = new File("/data/adb/zeromount/flags/zygisk_status");
        try (FileOutputStream os = new FileOutputStream(f)) {
            os.write(status.getBytes());
        } catch (Exception e) {
            Log.e(TAG, "status write failed", e);
        }
    }
}

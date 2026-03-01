package com.zeromount.hook;

import static com.v7878.unsafe.Reflection.getDeclaredMethod;
import static com.v7878.unsafe.invoke.EmulatedStackFrame.RETURN_VALUE_IDX;

import android.content.ContentProvider;
import android.database.MatrixCursor;
import android.net.Uri;
import android.os.Binder;
import android.os.Bundle;
import android.os.CancellationSignal;
import android.util.Log;

import com.v7878.unsafe.invoke.EmulatedStackFrame;
import com.v7878.unsafe.invoke.Transformers;
import com.v7878.vmtools.Hooks;
import com.v7878.vmtools.Hooks.EntryPointType;

import java.lang.reflect.Method;
import java.util.List;
import java.util.Set;

public class SettingsHook {
    private static final int FIRST_APP_UID = 10000;

    private static final Set<String> DEV_KEYS = Set.of(
        "adb_enabled",
        "development_settings_enabled",
        "adb_wifi_enabled",
        "hidden_api_policy",
        "allow_mock_location"
    );

    private static final Set<String> SUPPRESS_KEYS = Set.of("hidden_api_policy");

    // call(): Frame = [receiver, authority, method, arg, extras]
    private static final int CALL_IDX_METHOD = 2;
    private static final int CALL_IDX_ARG = 3;

    // query(): Frame = [receiver, uri, projection, queryArgs, cancellationSignal]
    private static final int QUERY_IDX_URI = 1;

    static void install() throws Throwable {
        // Virtual dispatch — Transport delegates to ContentProvider.this.call/query
        // Interface dispatch (IContentProvider) is bypassed by ART IMT/JIT
        Method callTarget = getDeclaredMethod(ContentProvider.class, "call",
            String.class, String.class, String.class, Bundle.class);
        Method queryTarget = getDeclaredMethod(ContentProvider.class, "query",
            Uri.class, String[].class, Bundle.class, CancellationSignal.class);

        Log.i(Main.TAG, "hooking " + callTarget);
        Log.i(Main.TAG, "hooking " + queryTarget);

        Hooks.hook(callTarget, EntryPointType.CURRENT, (original, frame) -> {
            if (shouldInterceptCall(frame)) {
                logInterceptCall(frame);
                spoofCallResult(frame);
                return;
            }
            Transformers.invokeExactWithFrame(original, frame);
        }, EntryPointType.DIRECT);

        Hooks.hook(queryTarget, EntryPointType.CURRENT, (original, frame) -> {
            String key = extractQueryKey(frame);
            if (key != null) {
                Log.i(Main.TAG, "intercept query " + key + " uid=" + Binder.getCallingUid());
                spoofQueryResult(frame, key);
                return;
            }
            Transformers.invokeExactWithFrame(original, frame);
        }, EntryPointType.DIRECT);
    }

    private static boolean shouldInterceptCall(EmulatedStackFrame frame) {
        var accessor = frame.accessor();

        String method = accessor.getReference(CALL_IDX_METHOD);
        if (method == null || !method.startsWith("GET_")) return false;

        String arg = accessor.getReference(CALL_IDX_ARG);
        if (arg == null || !DEV_KEYS.contains(arg)) return false;

        return Binder.getCallingUid() >= FIRST_APP_UID;
    }

    private static String extractQueryKey(EmulatedStackFrame frame) {
        if (Binder.getCallingUid() < FIRST_APP_UID) return null;

        Uri uri = frame.accessor().getReference(QUERY_IDX_URI);
        if (uri == null || !"settings".equals(uri.getAuthority())) return null;

        List<String> segments = uri.getPathSegments();
        if (segments.size() < 2) return null;

        String key = segments.get(1);
        return DEV_KEYS.contains(key) ? key : null;
    }

    private static void logInterceptCall(EmulatedStackFrame frame) {
        var accessor = frame.accessor();
        String method = accessor.getReference(CALL_IDX_METHOD);
        String arg = accessor.getReference(CALL_IDX_ARG);
        int uid = Binder.getCallingUid();
        String action = SUPPRESS_KEYS.contains(arg) ? "suppress" : "spoof->0";
        Log.i(Main.TAG, "intercept " + method + " " + arg + " uid=" + uid + " " + action);
    }

    // _generation_index=-1 prevents NameValueCache from caching real value client-side
    private static void spoofCallResult(EmulatedStackFrame frame) {
        String arg = frame.accessor().getReference(CALL_IDX_ARG);
        Bundle b = new Bundle();
        b.putString("value", SUPPRESS_KEYS.contains(arg) ? null : "0");
        b.putInt("_generation_index", -1);
        frame.accessor().setValue(RETURN_VALUE_IDX, b);
    }

    private static void spoofQueryResult(EmulatedStackFrame frame, String key) {
        String value = SUPPRESS_KEYS.contains(key) ? null : "0";
        MatrixCursor cursor = new MatrixCursor(new String[]{"name", "value"}, 1);
        cursor.addRow(new Object[]{key, value});
        frame.accessor().setValue(RETURN_VALUE_IDX, cursor);
    }
}

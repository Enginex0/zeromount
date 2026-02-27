#include "settings_hook.h"
#include <android/log.h>
#include <dlfcn.h>
#include <string.h>
#include <string>
#include <string_view>
#include <functional>

#include "lsplant.hpp"
#include "dobby.h"

#define TAG "ZeroMount-Settings"
#define LOGE(...) __android_log_print(ANDROID_LOG_ERROR, TAG, __VA_ARGS__)
#define LOGI(...) __android_log_print(ANDROID_LOG_INFO,  TAG, __VA_ARGS__)
#define LOGD(...) __android_log_print(ANDROID_LOG_DEBUG, TAG, __VA_ARGS__)

#define FIRST_APPLICATION_UID 10000
#define AID_ROOT   0
#define AID_SYSTEM 1000
#define AID_SHELL  2000

static const char *kAdbKeys[] = {
    "adb_enabled",
    "development_settings_enabled",
    "adb_wifi_enabled",
    nullptr,
};

// Globals retained for the lifetime of system_server after hook installation
static jclass    g_binder_class    = nullptr;
static jmethodID g_calling_uid_mid = nullptr;

static bool is_adb_key(const char *key) {
    for (int i = 0; kAdbKeys[i]; i++) {
        if (strcmp(key, kAdbKeys[i]) == 0) return true;
    }
    return false;
}

// Returns true when the current Binder transaction's caller UID is an
// untrusted app and should receive spoofed ADB settings.
static bool should_spoof_caller(JNIEnv *env) {
    jint uid = env->CallStaticIntMethod(g_binder_class, g_calling_uid_mid);
    if (env->ExceptionCheck()) {
        env->ExceptionClear();
        return false;
    }
    if (uid == AID_ROOT || uid == AID_SYSTEM || uid == AID_SHELL) return false;
    return uid >= FIRST_APPLICATION_UID;
}

// LSPlant callback: replaces SettingsProvider$NameValueCache.getStringForUser.
//
// LSPlant callback signature contract:
//   public Object callback(Object[] args)
//
// For a non-static target method, args layout is:
//   args[0] = receiver (NameValueCache instance)
//   args[1] = ContentResolver resolver
//   args[2] = String name  (the settings key)
//   args[3] = Integer userHandle
//
// The 'self' parameter is the hooker object (contains the backup Method field).
static jobject settings_callback(JNIEnv *env, jobject self, jobjectArray args);

// Resolve a symbol from libart.so. LSPlant requires this to hook ART internals.
static void *art_resolver(std::string_view name) {
    static void *libart = nullptr;
    if (!libart) {
        libart = dlopen("libart.so", RTLD_NOW | RTLD_GLOBAL | RTLD_NOLOAD);
        if (!libart) libart = dlopen("libart.so", RTLD_NOW | RTLD_GLOBAL);
    }
    if (!libart) return nullptr;
    return dlsym(libart, std::string(name).c_str());
}

// Dobby is statically linked — no dlopen needed
static void *inline_hook(void *target, void *hooker) {
    void *origin = nullptr;
    if (DobbyHook(target, hooker, &origin) == 0) return origin;
    return nullptr;
}

static bool inline_unhook(void *func) {
    return DobbyDestroy(func) == 0;
}

// Load a class from the on-disk hooker dex bundled alongside the module.
// DexClassLoader is available in system_server (it loads arbitrary dex files).
static jclass load_hooker_class(JNIEnv *env) {
    jclass dcl_class = env->FindClass("dalvik/system/DexClassLoader");
    if (!dcl_class) { env->ExceptionClear(); return nullptr; }

    jmethodID dcl_ctor = env->GetMethodID(
        dcl_class, "<init>",
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;"
        "Ljava/lang/ClassLoader;)V");

    jclass loader_class = env->FindClass("java/lang/ClassLoader");
    jmethodID get_syscl = env->GetStaticMethodID(
        loader_class, "getSystemClassLoader", "()Ljava/lang/ClassLoader;");
    jobject sys_cl = env->CallStaticObjectMethod(loader_class, get_syscl);

    // Module dex is installed next to the .so in the module directory
    jstring dex_path  = env->NewStringUTF(
        "/data/adb/modules/meta-zeromount/zygisk/hooker.dex");
    jstring opt_dir   = env->NewStringUTF(
        "/data/adb/modules/meta-zeromount/zygisk");

    jobject dcl = env->NewObject(dcl_class, dcl_ctor,
                                 dex_path, opt_dir, nullptr, sys_cl);
    if (env->ExceptionCheck() || !dcl) {
        env->ExceptionClear();
        LOGE("DexClassLoader for hooker.dex failed");
        return nullptr;
    }

    jmethodID load_class = env->GetMethodID(
        dcl_class, "loadClass",
        "(Ljava/lang/String;)Ljava/lang/Class;");
    jstring class_name = env->NewStringUTF("com.zeromount.hook.SettingsHooker");
    jclass hooker = reinterpret_cast<jclass>(
        env->CallObjectMethod(dcl, load_class, class_name));
    if (env->ExceptionCheck() || !hooker) {
        env->ExceptionClear();
        LOGE("loadClass(SettingsHooker) failed");
        return nullptr;
    }
    return reinterpret_cast<jclass>(env->NewGlobalRef(hooker));
}

bool install_settings_hook(JNIEnv *env) {
    // Cache Binder.getCallingUid() for the callback
    jclass binder = env->FindClass("android/os/Binder");
    if (!binder) { env->ExceptionClear(); LOGE("Binder class not found"); return false; }
    jmethodID uid_mid = env->GetStaticMethodID(binder, "getCallingUid", "()I");
    if (!uid_mid) { env->ExceptionClear(); LOGE("getCallingUid not found"); return false; }
    g_binder_class    = reinterpret_cast<jclass>(env->NewGlobalRef(binder));
    g_calling_uid_mid = uid_mid;

    // Locate the Settings$NameValueCache.getStringForUser hook target
    jclass cache_class = env->FindClass("android/provider/Settings$NameValueCache");
    if (!cache_class) {
        env->ExceptionClear();
        LOGE("Settings$NameValueCache not found — SettingsProvider not yet loaded");
        return false;
    }

    // Verify hook point exists before investing in LSPlant init
    jmethodID verify_mid = env->GetMethodID(
        cache_class, "getStringForUser",
        "(Landroid/content/ContentResolver;Ljava/lang/String;I)Ljava/lang/String;");
    if (!verify_mid) {
        env->ExceptionClear();
        LOGE("getStringForUser not found — Android API version mismatch?");
        env->DeleteLocalRef(cache_class);
        return false;
    }
    LOGI("Hook target verified: Settings$NameValueCache.getStringForUser");

    lsplant::InitInfo init_info{
        .inline_hooker              = inline_hook,
        .inline_unhooker            = inline_unhook,
        .art_symbol_resolver        = art_resolver,
        .art_symbol_prefix_resolver = nullptr,
    };
    if (!lsplant::Init(env, init_info)) {
        LOGE("lsplant::Init failed");
        env->DeleteLocalRef(cache_class);
        return false;
    }
    LOGI("LSPlant initialized");

    // Load the hooker class from the bundled dex
    jclass hooker_class = load_hooker_class(env);
    if (!hooker_class) {
        env->DeleteLocalRef(cache_class);
        return false;
    }

    // Register our C callback as the native implementation of SettingsHooker.getStringForUser
    JNINativeMethod native_cb{
        "getStringForUser",
        "([Ljava/lang/Object;)Ljava/lang/Object;",
        reinterpret_cast<void *>(settings_callback),
    };
    if (env->RegisterNatives(hooker_class, &native_cb, 1) != 0) {
        env->ExceptionClear();
        LOGE("RegisterNatives for SettingsHooker failed");
        env->DeleteLocalRef(cache_class);
        return false;
    }

    // Instantiate the hooker object
    jmethodID ctor = env->GetMethodID(hooker_class, "<init>", "()V");
    jobject hooker_obj = env->NewObject(hooker_class, ctor);
    if (env->ExceptionCheck() || !hooker_obj) {
        env->ExceptionClear();
        LOGE("SettingsHooker instantiation failed");
        env->DeleteLocalRef(cache_class);
        return false;
    }

    // Obtain java.lang.reflect.Method objects required by lsplant::Hook.
    // getDeclaredMethod is unrestricted in system_server (no hidden API enforcement).
    jclass class_clazz = env->FindClass("java/lang/Class");
    jmethodID get_decl = env->GetMethodID(
        class_clazz, "getDeclaredMethod",
        "(Ljava/lang/String;[Ljava/lang/Class;)Ljava/lang/reflect/Method;");

    // Param types for getStringForUser(ContentResolver, String, int)
    jclass cr_class  = env->FindClass("android/content/ContentResolver");
    jclass str_class = env->FindClass("java/lang/String");
    jclass int_class = env->FindClass("java/lang/Integer");
    jclass int_prim  = reinterpret_cast<jclass>(
        env->GetStaticObjectField(int_class,
            env->GetStaticFieldID(int_class, "TYPE", "Ljava/lang/Class;")));

    jobjectArray target_params = env->NewObjectArray(3, class_clazz, nullptr);
    env->SetObjectArrayElement(target_params, 0, cr_class);
    env->SetObjectArrayElement(target_params, 1, str_class);
    env->SetObjectArrayElement(target_params, 2, int_prim);

    jobject target_method = env->CallObjectMethod(
        cache_class, get_decl,
        env->NewStringUTF("getStringForUser"),
        target_params);
    if (env->ExceptionCheck() || !target_method) {
        env->ExceptionClear();
        LOGE("getDeclaredMethod(getStringForUser) failed");
        env->DeleteLocalRef(cache_class);
        return false;
    }

    // Param types for SettingsHooker.getStringForUser(Object[])
    jclass obj_arr_class = env->FindClass("[Ljava/lang/Object;");
    jobjectArray cb_params = env->NewObjectArray(1, class_clazz, nullptr);
    env->SetObjectArrayElement(cb_params, 0, obj_arr_class);

    jobject cb_method = env->CallObjectMethod(
        hooker_class, get_decl,
        env->NewStringUTF("getStringForUser"),
        cb_params);
    if (env->ExceptionCheck() || !cb_method) {
        env->ExceptionClear();
        LOGE("getDeclaredMethod(hooker callback) failed");
        env->DeleteLocalRef(cache_class);
        return false;
    }

    // Install the hook
    jobject backup = lsplant::Hook(env, target_method, hooker_obj, cb_method);
    if (!backup) {
        LOGE("lsplant::Hook returned null");
        env->DeleteLocalRef(cache_class);
        return false;
    }

    // Store backup in hooker object so the callback can call the original method
    jfieldID backup_fid = env->GetFieldID(
        hooker_class, "backup", "Ljava/lang/reflect/Method;");
    env->SetObjectField(hooker_obj, backup_fid, backup);

    LOGI("Hook installed — ADB settings spoofed for untrusted apps");
    env->DeleteLocalRef(cache_class);
    return true;
}

// LSPlant callback for Settings$NameValueCache.getStringForUser.
//
// args layout (non-static method):
//   args[0] = NameValueCache instance (receiver)
//   args[1] = ContentResolver
//   args[2] = String name
//   args[3] = Integer userHandle (auto-boxed)
static jobject settings_callback(JNIEnv *env, jobject self, jobjectArray args) {
    // Retrieve backup method from the hooker object
    jfieldID fid = env->GetFieldID(
        env->GetObjectClass(self), "backup", "Ljava/lang/reflect/Method;");
    jobject backup = env->GetObjectField(self, fid);

    // Invoke original to get the real value
    jclass method_class = env->FindClass("java/lang/reflect/Method");
    jmethodID invoke_mid = env->GetMethodID(
        method_class, "invoke",
        "(Ljava/lang/Object;[Ljava/lang/Object;)Ljava/lang/Object;");

    jobject receiver = env->GetObjectArrayElement(args, 0);
    jint    arg_len  = env->GetArrayLength(args);

    // Build the parameter array for Method.invoke (excludes receiver at args[0])
    jobjectArray invoke_args = env->NewObjectArray(
        arg_len - 1, env->FindClass("java/lang/Object"), nullptr);
    for (jint i = 1; i < arg_len; i++) {
        env->SetObjectArrayElement(invoke_args, i - 1,
                                   env->GetObjectArrayElement(args, i));
    }

    jobject real_result = env->CallObjectMethod(backup, invoke_mid, receiver, invoke_args);
    if (env->ExceptionCheck()) {
        env->ExceptionClear();
        return real_result;
    }

    // args[2] = String name (settings key)
    if (arg_len < 3) return real_result;
    jobject key_obj = env->GetObjectArrayElement(args, 2);
    if (!key_obj) return real_result;

    const char *key = env->GetStringUTFChars(reinterpret_cast<jstring>(key_obj), nullptr);
    if (!key) return real_result;
    bool intercept = is_adb_key(key);
    env->ReleaseStringUTFChars(reinterpret_cast<jstring>(key_obj), key);

    if (!intercept) return real_result;

    // ADB key — spoof for untrusted callers only
    if (!should_spoof_caller(env)) return real_result;

    LOGD("Intercepted ADB setting query — returning 0 to untrusted app");
    return env->NewStringUTF("0");
}

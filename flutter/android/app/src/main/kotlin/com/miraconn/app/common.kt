package com.miraconn.app

import android.annotation.SuppressLint
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.provider.Settings.*
import android.util.Log
import com.hjq.permissions.XXPermissions
import ffi.FFI
import java.util.*

// Flutter channel
const val START_ACTION = "start_action"
const val GET_START_ON_BOOT_OPT = "get_start_on_boot_opt"
const val SET_START_ON_BOOT_OPT = "set_start_on_boot_opt"
const val SYNC_APP_DIR_CONFIG_PATH = "sync_app_dir"
const val GET_VALUE = "get_value"

const val KEY_IS_SUPPORT_VOICE_CALL = "KEY_IS_SUPPORT_VOICE_CALL"

const val KEY_SHARED_PREFERENCES = "KEY_SHARED_PREFERENCES"
const val KEY_START_ON_BOOT_OPT = "KEY_START_ON_BOOT_OPT"
const val KEY_APP_DIR_CONFIG_PATH = "KEY_APP_DIR_CONFIG_PATH"

@SuppressLint("ConstantLocale")
val LOCAL_NAME = Locale.getDefault().toString()

fun requestPermission(context: Context, type: String) {
    XXPermissions.with(context)
        .permission(type)
        .request { _, all ->
            if (all) {
                Handler(Looper.getMainLooper()).post {
                    MainActivity.flutterMethodChannel?.invokeMethod(
                        "on_android_permission_result",
                        mapOf("type" to type, "result" to all)
                    )
                }
            }
        }
}

fun startAction(context: Context, action: String) {
    try {
        context.startActivity(Intent(action).apply {
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            // don't pass package name when launch ACTION_ACCESSIBILITY_SETTINGS
            if (ACTION_ACCESSIBILITY_SETTINGS != action) {
                data = Uri.parse("package:" + context.packageName)
            }
        })
    } catch (e: Exception) {
        e.printStackTrace()
    }
}

 fun translate(input: String): String {
    Log.d("common", "translate:$LOCAL_NAME")
    return FFI.translateLocale(LOCAL_NAME, input)
}
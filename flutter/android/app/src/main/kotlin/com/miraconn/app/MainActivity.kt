package com.miraconn.app

/**
 * Handle events from flutter
 * Request MediaProjection permission
 *
 * Inspired by [droidVNC-NG] https://github.com/bk138/droidVNC-NG
 */

import ffi.FFI

import android.content.Context
import android.os.Bundle
import android.view.WindowManager
import com.hjq.permissions.XXPermissions
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel


class MainActivity : FlutterActivity() {
    companion object {
        var flutterMethodChannel: MethodChannel? = null
    }

    private val channelTag = "mChannel"

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        flutterMethodChannel = MethodChannel(
            flutterEngine.dartExecutor.binaryMessenger,
            channelTag
        )
        initFlutterChannel(flutterMethodChannel!!)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // 初始化 ndk_context，hwcodec / TLS 等 native 组件依赖此上下文
        FFI.init(this)
    }

    private fun initFlutterChannel(flutterMethodChannel: MethodChannel) {
        flutterMethodChannel.setMethodCallHandler { call, result ->
            // make sure result will be invoked, otherwise flutter will await forever
            when (call.method) {
                "check_permission" -> {
                    if (call.arguments is String) {
                        result.success(XXPermissions.isGranted(context, call.arguments as String))
                    } else {
                        result.success(false)
                    }
                }
                "request_permission" -> {
                    if (call.arguments is String) {
                        requestPermission(context, call.arguments as String)
                        result.success(true)
                    } else {
                        result.success(false)
                    }
                }
                START_ACTION -> {
                    if (call.arguments is String) {
                        startAction(context, call.arguments as String)
                        result.success(true)
                    } else {
                        result.success(false)
                    }
                }
                "enable_soft_keyboard" -> {
                    // https://blog.csdn.net/hanye2020/article/details/105553780
                    if (call.arguments as Boolean) {
                        window.clearFlags(WindowManager.LayoutParams.FLAG_ALT_FOCUSABLE_IM)
                    } else {
                        window.addFlags(WindowManager.LayoutParams.FLAG_ALT_FOCUSABLE_IM)
                    }
                    result.success(true)
                }
                GET_START_ON_BOOT_OPT -> {
                    val prefs = getSharedPreferences(KEY_SHARED_PREFERENCES, MODE_PRIVATE)
                    result.success(prefs.getBoolean(KEY_START_ON_BOOT_OPT, false))
                }
                SET_START_ON_BOOT_OPT -> {
                    if (call.arguments is Boolean) {
                        val prefs = getSharedPreferences(KEY_SHARED_PREFERENCES, MODE_PRIVATE)
                        val edit = prefs.edit()
                        edit.putBoolean(KEY_START_ON_BOOT_OPT, call.arguments as Boolean)
                        edit.apply()
                        result.success(true)
                    } else {
                        result.success(false)
                    }
                }
                SYNC_APP_DIR_CONFIG_PATH -> {
                    if (call.arguments is String) {
                        val prefs = getSharedPreferences(KEY_SHARED_PREFERENCES, MODE_PRIVATE)
                        val edit = prefs.edit()
                        edit.putString(KEY_APP_DIR_CONFIG_PATH, call.arguments as String)
                        edit.apply()
                        result.success(true)
                    } else {
                        result.success(false)
                    }
                }
                GET_VALUE -> {
                    if (call.arguments is String) {
                        if (call.arguments == KEY_IS_SUPPORT_VOICE_CALL) {
                            result.success(false)
                        } else {
                            result.error("-1", "No such key", null)
                        }
                    } else {
                        result.success(null)
                    }
                }
                else -> {
                    result.error("-1", "No such method", null)
                }
            }
        }
    }
}

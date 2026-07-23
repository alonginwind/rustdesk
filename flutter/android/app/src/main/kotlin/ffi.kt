// ffi.kt

package ffi

import android.content.Context

object FFI {
    init {
        System.loadLibrary("miraconn")
    }

    external fun init(ctx: Context)
    external fun translateLocale(localeName: String, input: String): String
    external fun getLocalOption(key: String): String
    external fun getBuildinOption(key: String): String
}

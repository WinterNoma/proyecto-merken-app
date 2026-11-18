package com.pulsys.app

import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

class MainActivity : FlutterActivity() {
    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        MethodChannel(
            flutterEngine.dartExecutor.binaryMessenger,
            "com.pulsys.app/environment",
        ).setMethodCallHandler { call, result ->
            if (call.method == "getEnvironment") {
                result.success(
                    mapOf(
                        "privateDirectory" to noBackupFilesDir.absolutePath,
                        "appVersion" to packageManager.getPackageInfo(packageName, 0).versionName,
                    ),
                )
            } else {
                result.notImplemented()
            }
        }
    }
}

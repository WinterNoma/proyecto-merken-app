package com.merken.slmassets

import android.app.Activity
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import java.security.MessageDigest
import java.util.concurrent.atomic.AtomicBoolean

@TauriPlugin
class SlmAssetsPlugin(private val activity: Activity) : Plugin(activity) {
    private val busy = AtomicBoolean(false)
    private val files = mapOf(
        "space-health-base-Q4_K_M.gguf" to "4fce4f8510d16a058a9e6eef90d6ffd992135259e0fcadd42e69b06d1243dea1",
        "wellbeing-v7-lora-f16.gguf" to "521290d5207de280a7fc1ebc7517138e2dd994405347cc451a16fa60d5ffc2ef",
        "space-health-lora-f16.gguf" to "ca3d2b23e39547d0ebafb2e47793b4fba3a2943978e59205328dac47f5dd750f",
        // Dictado (Whisper base int8): el servicio de voz lo busca en <raíz>/stt-whisper-base/.
        "stt-whisper-base/base-encoder.int8.onnx" to "0b8fb1304b6109976038efff5ace81720e00386f3ff6b54ee8c75291ca0a1e11",
        "stt-whisper-base/base-decoder.int8.onnx" to "9759d217388a01b3a4c7c15533201067b48ae819c4daafc8624e64b9409dc02d",
        "stt-whisper-base/base-tokens.txt" to "b34b360dbb493e781e479794586d661700670d65564001f23024971d1f2fa126"
    )
    private fun bundled(path: String): Boolean {
        val parent = if (path.contains('/')) "slm/" + path.substringBeforeLast('/') else "slm"
        return activity.assets.list(parent)?.contains(path.substringAfterLast('/')) == true
    }
    private val base = "space-health-base-Q4_K_M.gguf"
    private fun hash(file: File): String {
        val digest = MessageDigest.getInstance("SHA-256")
        file.inputStream().use { input ->
            val buffer = ByteArray(512 * 1024)
            while (true) { val n = input.read(buffer); if (n < 0) break; digest.update(buffer, 0, n) }
        }
        return digest.digest().joinToString("") { "%02x".format(it.toInt() and 255) }
    }
    // Ruta fija sin copiar nada: el servicio SLM (v5) la usa como raíz de modelos.
    @Command
    fun directory(invoke: Invoke) {
        val result = JSObject(); result.put("directory", File(activity.filesDir, "slm-v7").absolutePath)
        invoke.resolve(result)
    }
    @Command
    fun prepare(invoke: Invoke) {
        if (!busy.compareAndSet(false, true)) { invoke.reject("Model preparation is already running"); return }
        Thread {
            try {
                val directory = File(activity.filesDir, "slm-v7"); directory.mkdirs()
                // Solo se instalan los modelos incluidos en esta APK (la base es obligatoria);
                // así una APK de prueba puede llevar base + v5 sin el adaptador V7.
                if (!bundled(base)) throw IllegalStateException("Base model is not bundled")
                for ((name, expected) in files) {
                    if (!bundled(name)) continue
                    val target = File(directory, name)
                    target.parentFile?.mkdirs()
                    if (target.isFile && hash(target) == expected) continue
                    val temporary = File(directory, "$name.pending")
                    activity.assets.open("slm/$name").use { input -> temporary.outputStream().use { output -> input.copyTo(output, 512 * 1024) } }
                    if (hash(temporary) != expected) { temporary.delete(); throw IllegalStateException("Bundled model integrity check failed") }
                    if (!temporary.renameTo(target)) throw IllegalStateException("Unable to install bundled offline model")
                }
                val result = JSObject(); result.put("directory", directory.absolutePath)
                invoke.resolve(result)
            } catch (_: Exception) {
                invoke.reject("Offline model preparation failed. Check storage and reinstall the complete APK.")
            } finally { busy.set(false) }
        }.start()
    }
}

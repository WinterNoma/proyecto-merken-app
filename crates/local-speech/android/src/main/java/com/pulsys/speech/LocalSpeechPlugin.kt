package com.pulsys.speech

import android.Manifest
import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.speech.RecognitionListener
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer
import androidx.appcompat.app.AppCompatActivity
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Channel
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class SpeechStart {
    lateinit var session: String
    lateinit var language: String
    lateinit var events: Channel
}

@InvokeArg
class SpeechFinish {
    lateinit var session: String
    var discard = false
}

@TauriPlugin(permissions = [Permission(strings = [Manifest.permission.RECORD_AUDIO], alias = "microphone")])
class LocalSpeechPlugin(private val activity: Activity) : Plugin(activity) {
    private val handler = Handler(Looper.getMainLooper())
    private var active: Session? = null
    private var permissionPending = false
    private var earlyFinish: String? = null

    private class Session(
        val args: SpeechStart,
        val invoke: Invoke,
        val recognizer: SpeechRecognizer,
    ) {
        var timeout: Runnable? = null
        var stopping = false
    }

    @Command
    fun start(invoke: Invoke) {
        val args = invoke.parseArgs(SpeechStart::class.java)
        activity.runOnUiThread {
            when {
                earlyFinish == args.session -> {
                    earlyFinish = null
                    invoke.reject("Start dictation again.")
                }
                active != null || permissionPending -> invoke.reject("Dictation is already in progress.")
                Build.VERSION.SDK_INT < Build.VERSION_CODES.S ||
                    !SpeechRecognizer.isOnDeviceRecognitionAvailable(activity) ->
                    invoke.reject("On-device dictation is unavailable on this device. You can type instead.")
                activity.checkSelfPermission(Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED -> {
                    permissionPending = true
                    requestPermissionForAlias("microphone", invoke, "microphonePermission")
                }
                else -> listen(args, invoke)
            }
        }
    }

    @PermissionCallback
    fun microphonePermission(invoke: Invoke) {
        permissionPending = false
        if (activity.checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED) {
            invoke.reject("Microphone enabled. Select Start dictation again.")
        } else {
            invoke.reject("Microphone permission was not granted. You can type instead.")
        }
    }

    private fun listen(args: SpeechStart, invoke: Invoke) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S) return
        val recognizer = try {
            SpeechRecognizer.createOnDeviceSpeechRecognizer(activity)
        } catch (_: RuntimeException) {
            invoke.reject("On-device dictation could not start. You can type instead.")
            return
        }
        val session = Session(args, invoke, recognizer)
        active = session
        val timeout = Runnable { complete(session, null, "Dictation timed out. You can try again or type.") }
        session.timeout = timeout
        handler.postDelayed(timeout, 25_000)
        val intent = Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH)
            .putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
            .putExtra(RecognizerIntent.EXTRA_LANGUAGE, args.language)
            .putExtra(RecognizerIntent.EXTRA_PARTIAL_RESULTS, true)
            .putExtra(RecognizerIntent.EXTRA_MAX_RESULTS, 1)
        recognizer.setRecognitionListener(object : RecognitionListener {
            override fun onReadyForSpeech(params: Bundle?) = emit(session, "listening")
            override fun onEndOfSpeech() = emit(session, "processing")
            override fun onPartialResults(results: Bundle?) {
                transcript(results)?.let { emit(session, "partial", it) }
            }
            override fun onResults(results: Bundle?) {
                complete(session, transcript(results), "No speech was recognized. Try again or type.")
            }
            override fun onError(error: Int) {
                val message = when (error) {
                    SpeechRecognizer.ERROR_LANGUAGE_NOT_SUPPORTED, SpeechRecognizer.ERROR_LANGUAGE_UNAVAILABLE ->
                        "On-device dictation is unavailable for this language. You can type instead."
                    SpeechRecognizer.ERROR_NO_MATCH, SpeechRecognizer.ERROR_SPEECH_TIMEOUT ->
                        "No speech was recognized. Try again or type."
                    SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS ->
                        "Microphone permission is required. You can type instead."
                    else -> "On-device dictation failed. Try again or type."
                }
                complete(session, null, message)
            }
            override fun onBeginningOfSpeech() {}
            override fun onRmsChanged(rmsdB: Float) {}
            override fun onBufferReceived(buffer: ByteArray?) {}
            override fun onEvent(eventType: Int, params: Bundle?) {}
        })
        try {
            recognizer.startListening(intent)
        } catch (_: RuntimeException) {
            complete(session, null, "On-device dictation could not start. You can type instead.")
        }
    }

    @Command
    fun finish(invoke: Invoke) {
        val args = invoke.parseArgs(SpeechFinish::class.java)
        activity.runOnUiThread {
            val session = active
            if (session == null) {
                earlyFinish = args.session
            } else if (session.args.session == args.session) {
                if (args.discard) complete(session, null, "Dictation canceled.")
                else if (!session.stopping) {
                    session.stopping = true
                    session.recognizer.stopListening()
                }
            }
            invoke.resolve()
        }
    }

    override fun onPause(activity: AppCompatActivity) {
        active?.let { complete(it, null, "Dictation canceled.") }
    }

    private fun emit(session: Session, state: String, text: String? = null) {
        if (active === session) {
            val event = JSObject().put("state", state)
            if (text != null) event.put("text", text)
            session.args.events.send(event)
        }
    }

    private fun complete(session: Session, text: String?, message: String) {
        if (active !== session) return
        active = null
        session.timeout?.let(handler::removeCallbacks)
        session.recognizer.destroy()
        if (text.isNullOrBlank()) session.invoke.reject(message)
        else session.invoke.resolveObject(text)
    }

    private fun transcript(results: Bundle?): String? =
        results?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)?.firstOrNull()?.trim()
}

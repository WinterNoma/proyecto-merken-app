import Flutter
import UIKit

@main
@objc class AppDelegate: FlutterAppDelegate, FlutterImplicitEngineDelegate {
  override func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
  ) -> Bool {
    return super.application(application, didFinishLaunchingWithOptions: launchOptions)
  }

  func didInitializeImplicitFlutterEngine(_ engineBridge: FlutterImplicitEngineBridge) {
    GeneratedPluginRegistrant.register(with: engineBridge.pluginRegistry)
    guard let registrar = engineBridge.pluginRegistry.registrar(forPlugin: "PulsysEnvironment")
    else { return }

    let channel = FlutterMethodChannel(
      name: "com.pulsys.app/environment",
      binaryMessenger: registrar.messenger()
    )
    channel.setMethodCallHandler { call, result in
      guard call.method == "getEnvironment" else {
        result(FlutterMethodNotImplemented)
        return
      }
      guard
        let directory = try? FileManager.default.url(
          for: .applicationSupportDirectory,
          in: .userDomainMask,
          appropriateFor: nil,
          create: true
        ),
        let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String
      else {
        result(FlutterError(
          code: "storage_error",
          message: "The private application directory is unavailable.",
          details: nil
        ))
        return
      }
      result(["privateDirectory": directory.path, "appVersion": version])
    }
  }
}

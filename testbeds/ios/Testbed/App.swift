import SwiftUI
import UIKit

@main
final class AppDelegate: UIResponder, UIApplicationDelegate {
    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
    ) -> Bool {
        Log.event("launch")
        return true
    }

    func application(
        _ application: UIApplication,
        configurationForConnecting connectingSceneSession: UISceneSession,
        options: UIScene.ConnectionOptions
    ) -> UISceneConfiguration {
        let config = UISceneConfiguration(name: nil, sessionRole: connectingSceneSession.role)
        config.delegateClass = SceneDelegate.self
        return config
    }
}

final class SceneDelegate: UIResponder, UIWindowSceneDelegate {
    var window: UIWindow?
    private let model = AppModel()

    func scene(_ scene: UIScene, willConnectTo session: UISceneSession, options: UIScene.ConnectionOptions) {
        guard let scene = scene as? UIWindowScene else { return }
        let window = LoggingWindow(windowScene: scene)
        let host = UIHostingController(rootView: RootView().environmentObject(model))
        host.view.isMultipleTouchEnabled = true
        window.rootViewController = host
        window.makeKeyAndVisible()
        self.window = window
        if let name = UserDefaults.standard.string(forKey: "tab"), let tab = Tab(rawValue: name) {
            model.select(tab)
        }
        options.urlContexts.forEach { open($0.url) }
    }

    func scene(_ scene: UIScene, openURLContexts contexts: Set<UIOpenURLContext>) {
        contexts.forEach { open($0.url) }
    }

    func sceneDidBecomeActive(_ scene: UIScene) {
        UIApplication.shared.isIdleTimerDisabled = true
        Log.event("lifecycle", ["state": "active"])
        Log.event("idle_timer", ["disabled": UIApplication.shared.isIdleTimerDisabled])
    }

    func sceneWillResignActive(_ scene: UIScene) {
        UIApplication.shared.isIdleTimerDisabled = false
        Log.event("lifecycle", ["state": "inactive"])
    }
    func sceneDidEnterBackground(_ scene: UIScene) { Log.event("lifecycle", ["state": "background"]) }
    func sceneWillEnterForeground(_ scene: UIScene) { Log.event("lifecycle", ["state": "foreground"]) }

    private func open(_ url: URL) {
        Log.event("url", ["url": url.absoluteString])
        if url.host == "tab", let tab = Tab(rawValue: url.lastPathComponent) {
            model.select(tab)
        }
    }
}

final class LoggingWindow: UIWindow {
    private var fingers: [ObjectIdentifier: Int] = [:]

    override func sendEvent(_ event: UIEvent) {
        if event.type == .touches, let touches = event.allTouches {
            for touch in touches where touch.phase != .stationary {
                let key = ObjectIdentifier(touch)
                let finger = fingers[key] ?? (0...).first { !fingers.values.contains($0) }!
                fingers[key] = finger
                let point = touch.location(in: self)
                Log.event("touch", [
                    "phase": Self.name(touch.phase),
                    "finger": finger,
                    "x": (point.x * 10).rounded() / 10,
                    "y": (point.y * 10).rounded() / 10,
                    "uptime": touch.timestamp,
                ])
                if touch.phase == .ended || touch.phase == .cancelled {
                    fingers[key] = nil
                }
            }
        }
        super.sendEvent(event)
    }

    private static func name(_ phase: UITouch.Phase) -> String {
        switch phase {
        case .began: return "began"
        case .moved: return "moved"
        case .ended: return "ended"
        case .cancelled: return "cancelled"
        default: return "other"
        }
    }
}

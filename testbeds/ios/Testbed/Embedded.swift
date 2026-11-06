import SwiftUI
import UIKit
import WebKit

struct SecureScreen: View {
    var body: some View {
        VStack(spacing: 24) {
            Text("Secure screen").font(.title2)
            CaptureProtected(text: "Secret 4242")
                .frame(height: 44)
                .accessibilityIdentifier("secure.label")
        }
        .padding()
    }
}

// iOS has no FLAG_SECURE. Content drawn inside a secure UITextField's text canvas is left out
// of screenshots and recordings; that canvas is the field's first subview.
struct CaptureProtected: UIViewRepresentable {
    let text: String

    func makeUIView(context: Context) -> UIView {
        let field = UITextField()
        field.isSecureTextEntry = true
        field.isUserInteractionEnabled = false
        let container = UIView()
        guard let canvas = field.subviews.first else { return container }
        let label = UILabel()
        label.text = text
        label.textAlignment = .center
        label.font = .preferredFont(forTextStyle: .title1)
        label.translatesAutoresizingMaskIntoConstraints = false
        canvas.translatesAutoresizingMaskIntoConstraints = false
        canvas.addSubview(label)
        container.addSubview(canvas)
        NSLayoutConstraint.activate([
            canvas.topAnchor.constraint(equalTo: container.topAnchor),
            canvas.bottomAnchor.constraint(equalTo: container.bottomAnchor),
            canvas.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            canvas.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            label.centerXAnchor.constraint(equalTo: canvas.centerXAnchor),
            label.centerYAnchor.constraint(equalTo: canvas.centerYAnchor),
        ])
        container.isAccessibilityElement = true
        container.accessibilityLabel = text
        return container
    }

    func updateUIView(_ view: UIView, context: Context) {}
}

struct WebScreen: View {
    var body: some View {
        WebForm().accessibilityIdentifier("web.view")
    }
}

struct WebForm: UIViewRepresentable {
    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeUIView(context: Context) -> WKWebView {
        let config = WKWebViewConfiguration()
        config.userContentController.add(context.coordinator, name: "putb")
        let view = WKWebView(frame: .zero, configuration: config)
        view.allowsBackForwardNavigationGestures = true
        if let url = Bundle.main.url(forResource: "form", withExtension: "html") {
            view.loadFileURL(url, allowingReadAccessTo: url)
        }
        return view
    }

    func updateUIView(_ view: WKWebView, context: Context) {}

    final class Coordinator: NSObject, WKScriptMessageHandler {
        func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
            guard let body = message.body as? [String: Any] else { return }
            Log.event("web", body)
        }
    }
}

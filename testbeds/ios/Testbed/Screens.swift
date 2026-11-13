import SwiftUI
import UserNotifications

struct TapsScreen: View {
    @State private var last = "none"
    @State private var dark = false
    private let columns = Array(repeating: GridItem(.flexible(), spacing: 8), count: 4)

    var body: some View {
        VStack(spacing: 12) {
            Text("Last tap: \(last)").accessibilityIdentifier("taps.last")
            Toggle("Dark theme", isOn: $dark)
                .accessibilityIdentifier("taps.dark")
                .onChange(of: dark) { _, on in Log.event("toggle", ["on": on]) }
            LazyVGrid(columns: columns, spacing: 8) {
                ForEach(0..<24, id: \.self) { i in
                    Button("B\(i)") {
                        last = "b\(i)"
                        Log.event("tap", ["id": "b\(i)"])
                    }
                    .frame(maxWidth: .infinity, minHeight: 56)
                    .background(Color.accentColor.opacity(0.12), in: RoundedRectangle(cornerRadius: 8))
                    .accessibilityIdentifier("taps.b\(i)")
                }
            }
        }
        .padding()
    }
}

struct TextScreen: View {
    @State private var plain = ""
    @State private var multi = ""
    @State private var password = ""

    var body: some View {
        Form {
            TextField("Plain", text: $plain).accessibilityIdentifier("text.plain")
            TextField("Multiline", text: $multi, axis: .vertical)
                .lineLimit(3...6)
                .accessibilityIdentifier("text.multi")
            SecureField("Password", text: $password).accessibilityIdentifier("text.password")
        }
        .onChange(of: plain) { _, value in Log.event("text", ["field": "plain", "value": value]) }
        .onChange(of: multi) { _, value in Log.event("text", ["field": "multi", "value": value]) }
        .onChange(of: password) { _, value in Log.event("text", ["field": "password", "length": value.count]) }
    }
}

final class VisibleRows: ObservableObject {
    private var rows = Set<Int>()
    private var pending: DispatchWorkItem?

    func appear(_ row: Int) { rows.insert(row); schedule() }
    func disappear(_ row: Int) { rows.remove(row); schedule() }

    private func schedule() {
        pending?.cancel()
        let work = DispatchWorkItem { [weak self] in
            guard let self, let first = self.rows.min(), let last = self.rows.max() else { return }
            Log.event("scroll", ["first": first, "last": last])
        }
        pending = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3, execute: work)
    }
}

struct ListScreen: View {
    @StateObject private var visible = VisibleRows()

    var body: some View {
        List(0..<1000, id: \.self) { i in
            Text("Row \(i)")
                .accessibilityIdentifier("list.row.\(i)")
                .onAppear { visible.appear(i) }
                .onDisappear { visible.disappear(i) }
        }
        .accessibilityIdentifier("list.table")
    }
}

struct GesturesScreen: View {
    var body: some View {
        Rectangle()
            .fill(Color(.systemGray6))
            .overlay(Text("Gesture pad").foregroundStyle(.secondary))
            .accessibilityElement(children: .combine)
            .accessibilityIdentifier("gestures.pad")
    }
}

struct BusyScreen: View {
    @State private var spinning = false

    var body: some View {
        VStack(spacing: 24) {
            ProgressView().accessibilityIdentifier("busy.progress")
            Image(systemName: "arrow.triangle.2.circlepath")
                .font(.system(size: 64))
                .rotationEffect(.degrees(spinning ? 360 : 0))
                .animation(.linear(duration: 1).repeatForever(autoreverses: false), value: spinning)
                .accessibilityIdentifier("busy.spinner")
            Text("Always busy")
        }
        .onAppear { spinning = true }
    }
}

struct DialogsScreen: View {
    @State private var alert = false

    var body: some View {
        VStack(spacing: 24) {
            Button("Show alert") { alert = true }.accessibilityIdentifier("dialogs.alert")
            Button("Request notifications") {
                UNUserNotificationCenter.current().requestAuthorization(options: [.alert]) { granted, _ in
                    Log.event("permission", ["name": "notifications", "granted": granted])
                }
            }
            .accessibilityIdentifier("dialogs.permission")
            Button("Notify in 8 s") {
                let content = UNMutableNotificationContent()
                content.title = "Testbed banner"
                content.body = "A tap under this banner must wait."
                let trigger = UNTimeIntervalNotificationTrigger(timeInterval: 8, repeats: false)
                UNUserNotificationCenter.current().add(
                    UNNotificationRequest(identifier: UUID().uuidString, content: content, trigger: trigger)
                )
                Log.event("notification", ["state": "scheduled"])
            }
            .accessibilityIdentifier("dialogs.notify")
            Button("Place order \u{00b7} $12.00") { Log.event("tap", ["id": "order"]) }
                .accessibilityIdentifier("dialogs.order")
        }
        .alert("Testbed alert", isPresented: $alert) {
            Button("OK") { Log.event("dialog", ["choice": "ok"]) }
            Button("Cancel", role: .cancel) { Log.event("dialog", ["choice": "cancel"]) }
        } message: {
            Text("Pick one")
        }
    }
}

struct NavScreen: View {
    var body: some View {
        NavigationStack {
            NavLevel(level: 0)
                .navigationDestination(for: Int.self) { NavLevel(level: $0) }
        }
    }
}

struct NavLevel: View {
    let level: Int

    var body: some View {
        VStack(spacing: 24) {
            Text("Level \(level)").accessibilityIdentifier("nav.label")
            if level < 2 {
                NavigationLink("Open level \(level + 1)", value: level + 1)
                    .accessibilityIdentifier("nav.open\(level + 1)")
            }
        }
        .navigationTitle(level == 0 ? "Nav root" : "Level \(level)")
        .onAppear { Log.event("nav", ["level": level]) }
    }
}

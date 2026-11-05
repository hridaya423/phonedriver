import SwiftUI

enum Tab: String, CaseIterable, Identifiable {
    case taps, text, list, gestures, busy, secure, dialogs, web, nav

    var id: String { rawValue }
    var title: String { rawValue.capitalized }

    var icon: String {
        switch self {
        case .taps: return "hand.tap"
        case .text: return "character.cursor.ibeam"
        case .list: return "list.bullet"
        case .gestures: return "hand.draw"
        case .busy: return "hourglass"
        case .secure: return "lock"
        case .dialogs: return "exclamationmark.bubble"
        case .web: return "globe"
        case .nav: return "arrow.turn.down.right"
        }
    }
}

final class AppModel: ObservableObject {
    @Published private(set) var tab: Tab = .taps

    func select(_ tab: Tab) {
        self.tab = tab
        Log.event("tab", ["name": tab.rawValue])
    }
}

struct RootView: View {
    @EnvironmentObject private var model: AppModel

    var body: some View {
        VStack(spacing: 0) {
            screen.frame(maxWidth: .infinity, maxHeight: .infinity)
            Divider()
            TabBar()
        }
    }

    @ViewBuilder private var screen: some View {
        switch model.tab {
        case .taps: TapsScreen()
        case .text: TextScreen()
        case .list: ListScreen()
        case .gestures: GesturesScreen()
        case .busy: BusyScreen()
        case .secure: SecureScreen()
        case .dialogs: DialogsScreen()
        case .web: WebScreen()
        case .nav: NavScreen()
        }
    }
}

// Nine tabs do not fit a system TabView on a phone; it would move five of them behind "More".
struct TabBar: View {
    @EnvironmentObject private var model: AppModel

    var body: some View {
        HStack(spacing: 0) {
            ForEach(Tab.allCases) { tab in
                Button {
                    model.select(tab)
                } label: {
                    VStack(spacing: 2) {
                        Image(systemName: tab.icon).font(.system(size: 17))
                        Text(tab.title).font(.system(size: 9))
                    }
                    .frame(maxWidth: .infinity, minHeight: 44)
                    .foregroundStyle(model.tab == tab ? Color.accentColor : Color.secondary)
                }
                .accessibilityIdentifier("tab.\(tab.rawValue)")
                .accessibilityLabel(tab.title)
                .accessibilityAddTraits(model.tab == tab ? .isSelected : [])
            }
        }
        .padding(.top, 4)
        .background(.bar)
    }
}

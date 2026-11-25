import SwiftUI

struct PickersScreen: View {
    private static let start: Date = {
        var parts = DateComponents()
        (parts.year, parts.month, parts.day, parts.hour, parts.minute) = (2026, 1, 15, 9, 5)
        return Calendar(identifier: .gregorian).date(from: parts) ?? Date()
    }()

    @State private var date = start
    @State private var time = start
    @State private var dateWheel = start
    @State private var timeWheel = start
    @State private var shown = "nothing picked"

    private func picked(_ field: String, _ value: Date, _ format: String) {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.calendar = Calendar(identifier: .gregorian)
        formatter.dateFormat = format
        let text = formatter.string(from: value)
        shown = "\(field) = \(text)"
        Log.event("picked", ["field": field, "value": text])
    }

    var body: some View {
        Form {
            Text(shown).accessibilityIdentifier("pickers.shown")
            DatePicker("Date", selection: $date, displayedComponents: .date)
                .datePickerStyle(.compact)
                .accessibilityIdentifier("pickers.date")
                .onChange(of: date) { _, value in picked("date", value, "yyyy-MM-dd") }
            DatePicker("Time", selection: $time, displayedComponents: .hourAndMinute)
                .datePickerStyle(.compact)
                .accessibilityIdentifier("pickers.time")
                .onChange(of: time) { _, value in picked("time", value, "HH:mm") }
            DatePicker("Date wheel", selection: $dateWheel, displayedComponents: .date)
                .datePickerStyle(.wheel)
                .accessibilityIdentifier("pickers.date_wheel")
                .onChange(of: dateWheel) { _, value in picked("date_wheel", value, "yyyy-MM-dd") }
            DatePicker("Time wheel", selection: $timeWheel, displayedComponents: .hourAndMinute)
                .datePickerStyle(.wheel)
                .accessibilityIdentifier("pickers.time_wheel")
                .onChange(of: timeWheel) { _, value in picked("time_wheel", value, "HH:mm") }
        }
    }
}

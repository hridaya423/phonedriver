import Foundation

// The file is opened with `O_APPEND`, so a host that truncates it to clear the log keeps
// receiving new lines. Deleting the file instead would orphan the open descriptor.
enum Log {
    private static let queue = DispatchQueue(label: "dev.phoneuse.testbed.log")
    private static let fd: Int32 = {
        let dir = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
        return open(dir.appendingPathComponent("putb.log").path, O_WRONLY | O_APPEND | O_CREAT, 0o644)
    }()

    static func event(_ name: String, _ fields: [String: Any] = [:]) {
        var object = fields
        object["ev"] = name
        object["t"] = (Date().timeIntervalSince1970 * 1000).rounded()
        guard var line = try? JSONSerialization.data(withJSONObject: object, options: [.sortedKeys]) else {
            return
        }
        line.append(0x0A)
        queue.async {
            line.withUnsafeBytes { _ = write(fd, $0.baseAddress, $0.count) }
        }
    }
}

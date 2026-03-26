import Foundation
import UIKit

let maxConnections = 100

typealias CompletionHandler = (Bool) -> Void

/// Represents log severity.
enum LogLevel {
    case debug
    case info
    case warning
    case error
}

/// Protocol for objects that can be serialized.
protocol Serializable {
    func toJson() -> [String: Any]
    func toJsonString() -> String
}

/// Base class with shared functionality.
class Base {
    let name: String

    /// Initialize with a name.
    init(name: String) {
        self.name = name
    }

    func description() -> String {
        return "\(type(of: self))(\(name))"
    }

    private func validate() {
        assert(!name.isEmpty)
    }
}

/// Manages a network connection.
class Connection: Base {
    var port: Int
    private var connected: Bool = false

    init(host: String, port: Int = 8080) {
        self.port = port
        super.init(name: host)
    }

    /// Establish the connection.
    func connect() async throws {
        print("Connecting to \(name):\(port)")
        connected = true
    }

    func disconnect() {
        connected = false
    }

    var isConnected: Bool {
        return connected
    }
}

struct Point {
    let x: Double
    let y: Double

    func distance(to other: Point) -> Double {
        let dx = x - other.x
        let dy = y - other.y
        return (dx * dx + dy * dy).squareRoot()
    }
}

extension String {
    func toSlug() -> String {
        return lowercased().replacingOccurrences(of: " ", with: "-")
    }
}

func processUsers(_ users: [Base]) -> [String] {
    return users.map { $0.description() }
}

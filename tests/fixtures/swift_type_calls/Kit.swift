enum Cache {
    static func open(_ key: String, timeout: Double = 5) async throws -> Int { 0 }
    static func wait(for name: String, then body: () -> Void) { body() }
    static func stop(_ id: Int) {}
}

enum Registry {
    static func create() -> Int { 0 }
}

final class Worker {
    func start() {}
}

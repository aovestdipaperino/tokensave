final class RemoteSource: SourceBase, DataSource {
    func items() async throws -> [String] { [] }
    func close() {}
}

actor CachedSource: DataSource {
    func items() async throws -> [String] { [] }
    nonisolated func close() {}
}

struct FileLoader: Loadable {
    func load() -> Int { 1 }
}

enum Mode: String, Loadable {
    case fast
    func load() -> Int { 0 }
}

struct Plain {}

extension Plain: Loadable {
    func load() -> Int { 2 }
}

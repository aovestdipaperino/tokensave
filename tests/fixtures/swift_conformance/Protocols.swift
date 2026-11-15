public protocol DataSource {
    func items() async throws -> [String]
    func close()
}

public protocol Loadable {
    func load() -> Int
}

public protocol Reloadable: Loadable {
    func reload()
}

open class SourceBase {}

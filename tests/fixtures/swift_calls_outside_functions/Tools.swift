enum Launcher {
    static func boot() -> Int { 0 }
}

enum Reporter {
    static func summarize(_ value: Int) {}
}

enum Formatter {
    static func title(for id: Int) -> String { "" }
}

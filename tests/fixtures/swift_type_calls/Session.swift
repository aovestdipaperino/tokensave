struct Session {
    let worker: Worker

    func run() async throws {
        let id = try await Cache.open(
            "example-key",
            timeout: 10
        )
        Cache.wait(for: "ready") {
            print("ready")
        }
        Cache
            .stop(id)
        _ = Registry.create()
        worker.start()
    }
}

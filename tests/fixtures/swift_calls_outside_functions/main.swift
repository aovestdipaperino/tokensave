import Foundation

let booted = Launcher.boot()
if booted > 0 {
    Reporter.summarize(booted)
}

struct Panel {
    var heading: String {
        Formatter.title(for: 1)
    }
}

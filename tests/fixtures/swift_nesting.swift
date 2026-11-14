func deep(x: Int) {
    if x > 0 {
        for i in 0..<x {
            if i > 5 {
                print("deep")
            }
        }
    }
}

func flat() {
    let a = 1
    print(a)
}

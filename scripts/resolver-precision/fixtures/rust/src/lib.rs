pub struct RustThing;

pub fn rust_target(_: RustThing) -> usize {
    1
}

pub fn first_rust_call() -> usize {
    rust_target(RustThing)
}

pub fn second_rust_call(value: RustThing) -> usize {
    rust_target(value)
}

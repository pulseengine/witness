// Swift — leap-year MC/DC probe.
//
// Same predicate as the C/C++/Rust/Zig/Go fixtures, in Swift
// idiomatic form. Swift's `&&`/`||` are short-circuit operators
// lowered (via swiftc → LLVM IR) similarly to clang. Expected
// wasm shape: `if/else` + 1 `br_if` per source decision — the
// v0.19 IfThen clustering target.
//
// We use a UInt32 global the runner mutates between calls plus
// a `@inline(never)` predicate to keep Swift from constant-
// folding the result. (Swift's optimiser is aggressive about
// integer-literal arithmetic.)

// Hold the runtime input in a class so we don't have to thread
// global-mutation/strict-concurrency annotations through the
// fixture — keeps the surface area minimal for the predicate.
final class YearBox {
    var value: UInt32 = 0
}
let yearBox = YearBox()

@_silgen_name("year_input_set")
public func year_input_set(_ value: UInt32) {
    yearBox.value = value
}

@_silgen_name("run")
public func run() -> Int32 {
    return leapYear(yearBox.value) ? 1 : 0
}

@inline(never)
func leapYear(_ y: UInt32) -> Bool {
    return (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

@main
struct Main {
    static func main() {}
}

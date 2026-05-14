// TinyGo — leap-year MC/DC probe.
//
// Same predicate as the C/Rust/Zig fixtures: `(y%4==0 && y%100!=0)
// || (y%400==0)`. Go's `&&`/`||` are short-circuit operators.
// TinyGo uses LLVM as its backend, so the wasm lowering is
// expected to look like clang's: `if/else` + 1 br_if per source
// decision (the shape v0.19's IfThen clustering targets).
//
// Build with: `tinygo build -target wasm-unknown -o leap.wasm -opt 1`
// (see build.sh).

package main

//go:wasmexport year_input_set
func year_input_set(v uint32) {
	yearInput = v
}

//go:wasmexport run
func run() bool {
	return leapYear(yearInput)
}

var yearInput uint32

//go:noinline
func leapYear(y uint32) bool {
	return (y%4 == 0 && y%100 != 0) || (y%400 == 0)
}

func main() {}

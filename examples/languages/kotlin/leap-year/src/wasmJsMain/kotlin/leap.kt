// Kotlin/Wasm — leap-year MC/DC probe.
//
// Same predicate as the other fixtures. Kotlin/Wasm targets the
// wasm-gc proposal (GC reference types), which is post-MVP and a
// different shape from the wasm-MVP modules witness was originally
// tuned for. Branch detection on wasm-gc depends on walrus's
// support level for the GC types in the module.

@JsExport
fun leapYear(y: Int): Boolean {
    return (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

@JsExport
fun runRow0(): Boolean = leapYear(2001)

@JsExport
fun runRow1(): Boolean = leapYear(2004)

@JsExport
fun runRow2(): Boolean = leapYear(2100)

@JsExport
fun runRow3(): Boolean = leapYear(2000)

fun main() {}

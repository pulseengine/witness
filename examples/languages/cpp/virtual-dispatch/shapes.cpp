// C++ virtual dispatch — MC/DC null-result probe.
//
// Demonstrates a property worth knowing: **virtual dispatch is
// not a Decision in MC/DC sense**. The vtable lookup lowers to
// `call_indirect` in wasm, which witness's branch-detection
// pass IGNORES (it only counts `br_if`, `if/else`, `br_table`).
//
// This fixture deliberately uses a 4-arm dispatch with no
// short-circuit logic — the only branches should come from
// the runtime dispatch path inside each derived `area()`. If
// the manifest shows non-zero Decisions, that means a
// predicate hid in one of the area() implementations; if it
// shows zero Decisions, our doctrine ("vtables are runtime
// dispatch, not MC/DC") holds.

#include <cstdint>
#include <cstdio>

struct Shape {
    virtual ~Shape() = default;
    virtual int area() const = 0;
};

struct Square : Shape {
    int side;
    Square(int s) : side(s) {}
    int area() const override { return side * side; }
};

struct Rect : Shape {
    int w, h;
    Rect(int w_, int h_) : w(w_), h(h_) {}
    int area() const override { return w * h; }
};

struct Triangle : Shape {
    int base, height;
    Triangle(int b, int h) : base(b), height(h) {}
    int area() const override { return (base * height) / 2; }
};

struct Circle : Shape {
    int radius;
    Circle(int r) : radius(r) {}
    int area() const override { return 3 * radius * radius; }
};

volatile int input_kind;

[[gnu::noinline]]
static int compute_area(const Shape* s) {
    return s->area();
}

__attribute__((export_name("run_square")))
int run_square() { input_kind = 0; Square sq{5}; return compute_area(&sq); }

__attribute__((export_name("run_rect")))
int run_rect() { input_kind = 1; Rect r{3, 4}; return compute_area(&r); }

__attribute__((export_name("run_triangle")))
int run_triangle() { input_kind = 2; Triangle t{6, 8}; return compute_area(&t); }

__attribute__((export_name("run_circle")))
int run_circle() { input_kind = 3; Circle c{5}; return compute_area(&c); }

int main() {
    std::printf("%d %d %d %d\n", run_square(), run_rect(), run_triangle(), run_circle());
    return 0;
}

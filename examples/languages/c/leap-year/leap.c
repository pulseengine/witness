// Volatile sink forces the load — without this, clang at -O1
// constant-folds the predicate result and emits zero branches.
volatile unsigned year_input;

static int leap_year(unsigned y) {
    return (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
}

__attribute__((export_name("run_row_0")))
int run_row_0(void) { year_input = 2001; return leap_year(year_input); }

__attribute__((export_name("run_row_1")))
int run_row_1(void) { year_input = 2004; return leap_year(year_input); }

__attribute__((export_name("run_row_2")))
int run_row_2(void) { year_input = 2100; return leap_year(year_input); }

__attribute__((export_name("run_row_3")))
int run_row_3(void) { year_input = 2000; return leap_year(year_input); }

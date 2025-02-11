#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
    
// #define as_int(x) (*(int64_t*)&(x))
// #define as_float(x) (*(double*)&(x))

// Perform a cast from double to int64_t in a way that doesn't violate strict aliasing rules
int64_t as_int(double x) { return *(int64_t*)&x; }
// Perform a cast from int64_t to double in a way that doesn't violate strict aliasing rules
double as_float(int64_t x) { return *(double*)&x; }

int64_t _cage_memcpy(int64_t dst, int64_t src, int64_t len) {
    memcpy((void*)dst, (void*)src, len * sizeof(int64_t));
    return 0;
}


int64_t _cage_round(int64_t x) {
    return as_int(round(as_float(x)));
}

int64_t _cage_floor(int64_t x) {
    return as_int(floor(as_float(x)));
}

int64_t _cage_ceil(int64_t x) {
    return as_int(ceil(as_float(x)));
}

int64_t _cage_malloc(int64_t cells) {
    return (int64_t)malloc(cells * sizeof(int64_t));
}

int64_t _cage_to_float(int64_t x) {
    // Convert the int64_t to a double
    return as_int((double)x);
}

int64_t _cage_to_int(int64_t x) {
    // Convert the double to an int64_t
    return (int64_t)as_float(x);
}

int64_t _cage_free(int64_t ptr) {
    free((void*)ptr);
    return 0;
}

int64_t _cage_idx(int64_t ptr, int64_t i) {
    return ptr + i * sizeof(int64_t);
}

int64_t _cage_deref(int64_t x) {
    return *(int64_t*)x;
}

int64_t _cage_debug(int64_t x) {
    printf("DEBUG: int=%lld, float=%f\n", x, as_float(x));
    return x;
}

int64_t _cage_fprint(int64_t x) {
    printf("%lf", as_float(x));
    return x;
}

int64_t _cage_lt(int64_t x, int64_t y) {
    return x < y;
}

int64_t _cage_le(int64_t x, int64_t y) {
    return x <= y;
}

int64_t _cage_gt(int64_t x, int64_t y) {
    return x > y;
}

int64_t _cage_ge(int64_t x, int64_t y) {
    return x >= y;
}

int64_t _cage_eq(int64_t x, int64_t y) {
    return x == y;
}

int64_t _cage_flt(int64_t x, int64_t y) {
    return as_float(x) < as_float(y);
}

int64_t _cage_fgt(int64_t x, int64_t y) {
    return as_float(x) > as_float(y);
}

int64_t _cage_feq(int64_t x, int64_t y) {
    return as_float(x) == as_float(y);
}

int64_t _cage_fadd(int64_t x, int64_t y) {
    return as_int(as_float(x) + as_float(y));
}

int64_t _cage_fsub(int64_t x, int64_t y) {
    return as_int(as_float(x) - as_float(y));
}

int64_t _cage_fmul(int64_t x, int64_t y) {
    return as_int(as_float(x) * as_float(y));
}

int64_t _cage_fdiv(int64_t x, int64_t y) {
    return as_int(as_float(x) / as_float(y));
}

int64_t _cage_frem(int64_t x, int64_t y) {
    return as_int(fmod(as_float(x), as_float(y)));
}

int64_t _cage_fneg(int64_t x) {
    return as_int(-as_float(x));
}

int64_t _cage_addi(int64_t x, int64_t y) {
    return x + y;
}

int64_t _cage_add(int64_t x, int64_t y) {
    return x + y;
}

int64_t _cage_sub(int64_t x, int64_t y) {
    return x - y;
}

int64_t _cage_mul(int64_t x, int64_t y) {
    return x * y;
}

int64_t _cage_div(int64_t x, int64_t y) {
    return x / y;
}

int64_t _cage_rem(int64_t x, int64_t y) {
    return x % y;
}

int64_t _cage_neg(int64_t x) {
    return -x;
}

int64_t _cage_putchar(int64_t x) {
    putchar(x);
    return x;
}

int64_t _cage_putc(int64_t x) {
    putchar(x);
    return x;
}

int64_t _cage_puti(int64_t x) {
    printf("%lld", x);
    return x;
}

int64_t _cage_puthex(int64_t x) {
    printf("%08llx", x);
    return x;
}

int64_t _cage_putflt(int64_t x) {
    printf("%lld", x);
    return x;
}

int64_t _cage_read() {
    return getchar();
}

int64_t _cage_putarr(int64_t ptr, int64_t len) {
    for (int i = 0; i < len; i++) {
        printf("%lld ", *(int64_t*)(ptr + i * sizeof(int64_t)));
    }
    return 0;
}

int64_t _cage_putln() {
    printf("\n");
    return 0;
}

int64_t _cage_puts(int64_t ptr) {
    printf("%s", (char*)ptr);
    return 0;
}

int64_t _cage_putsln(int64_t ptr) {
    printf("%s\n", (char*)ptr);
    return 0;
}


int64_t _cage_srand(int64_t x) {
    srand(x);
    return 0;
}

int64_t _cage_rand(int64_t lower, int64_t upper) {
    return (rand() % (upper - lower + 1)) + lower;
}
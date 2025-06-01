#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int64_t _memcpy(int64_t dst, int64_t src, int64_t len) {
    memcpy((void*)dst, (void*)src, len * sizeof(int64_t));
    return 0;
}

int64_t _add(int64_t x, int64_t y) {
    return x + y;
}

int64_t _sub(int64_t x, int64_t y) {
    return x - y;
}

int64_t _mul(int64_t x, int64_t y) {
    return x * y;
}

int64_t _div(int64_t x, int64_t y) {
    return x / y;
}

int64_t _rem(int64_t x, int64_t y) {
    return x % y;
}

int64_t _neg(int64_t x) {
    return -x;
}

int64_t _putln() {
    return putchar('\n');
}

int64_t _putchar(int64_t x) {
    return putchar(x);
}

int64_t _putint(int64_t x) {
    return printf("%lld", x);
}

int64_t _puthex(int64_t x) {
    return printf("0x%016llx", x);
}

int64_t _putcstr(int64_t x) {
    return printf("%s", (char*)x);
}

int64_t _putstr(int64_t x) {
    int64_t *ptr = (int64_t*)x;
    while (*ptr) {
        _putchar(*ptr);
        ptr++;
    }
    return 0;
}


int64_t _lt(int64_t x, int64_t y) {
    return x < y;
}

int64_t _le(int64_t x, int64_t y) {
    return x <= y;
}

int64_t _gt(int64_t x, int64_t y) {
    return x > y;
}

int64_t _ge(int64_t x, int64_t y) {
    return x >= y;
}

int64_t _eq(int64_t x, int64_t y) {
    return x == y;
}

int64_t _ne(int64_t x, int64_t y) {
    return x != y;
}

int64_t _bitxor(int64_t x, int64_t y) {
    return x ^ y;
}

int64_t _bitand(int64_t x, int64_t y) {
    return x & y;
}   

int64_t _bitor(int64_t x, int64_t y) {
    return x | y;
}

int64_t _bitnot(int64_t x) {
    return ~x;
}

int64_t _streq(int64_t x, int64_t y) {
    int64_t *a = (int64_t*)x;
    int64_t *b = (int64_t*)y;
    while (*a && *b) {
        if (*a != *b) {
            return 0;
        }
        a++;
        b++;
    }
    return *a == *b;
}

int64_t _malloc(int64_t cells) {
    return (int64_t)malloc(cells * sizeof(int64_t));
}
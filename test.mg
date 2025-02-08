extern fun add(a, b);
extern fun sub(a, b);
extern fun puts(value);
extern fun puthex(value);
extern fun putarr(ptr, len);
extern fun putchar(value);
extern fun putln();
extern fun deref(value);
extern fun idx(ptr, i);
extern fun memcpy(dst, src, count);
extern fun malloc(size);

let static STACK = 0;
STACK = malloc(1024);
let static SP = 0;

fun new_scope() {
    return SP;
}

fun push(value) {
    SP = add(SP, 1);
    idx(STACK, SP) = value;
}

fun pop() {
    let value = deref(idx(STACK, SP));
    SP = add(SP, -1);
    return value;
}

fun poparr(ptr, count) {
    while (count) {
        count = add(count, -1);
        idx(ptr, count) = pop();
    }
}

fun pusharr(ptr, count) {
    let i = 0;
    while (count) {
        count = add(count, -1);
        push(deref(idx(ptr, i)));
        i = add(i, 1);
    }
}

fun ret(ebp, count) {
    let current_sp = SP;
    // Revert stack pointer
    SP = add(ebp, count);
    // Copy return values to stack
    memcpy(idx(STACK, ebp), idx(STACK, current_sp), count);
} 


    push(1);
    push(2);
    push(3);
    let p = idx(STACK, add(SP, -2));
extern fun puti(n);
extern fun putc(c);
extern fun addi(X, Y);
fun test() {  let y = idx(STACK, add(SP, -0));
  let x = idx(STACK, add(SP, -1));
   let ebp = new_scope();
    pusharr(x, 1);
    pusharr(y, 1);
    let __EXTERN__YID_25 = pop();
    let __EXTERN__XID_25 = pop();
    push(addi(__EXTERN__XID_25, __EXTERN__YID_25));
    let z = idx(STACK, add(SP, -0));
    pusharr(z, 1);
    ret(ebp, 1);
}
fun test2() {  let p = idx(STACK, add(SP, -2));
   let ebp = new_scope();
    pusharr(idx(p, 0), 1);
    let __EXTERN__nID_33 = pop();
    push(puti(__EXTERN__nID_33));
    push(' ');
    let __EXTERN__cID_34 = pop();
    push(putc(__EXTERN__cID_34));
    pusharr(idx(p, 1), 1);
    let __EXTERN__nID_35 = pop();
    push(puti(__EXTERN__nID_35));
    push(' ');
    let __EXTERN__cID_36 = pop();
    push(putc(__EXTERN__cID_36));
    pusharr(idx(p, 2), 1);
    let __EXTERN__nID_37 = pop();
    push(puti(__EXTERN__nID_37));
    push('\n');
    let __EXTERN__cID_38 = pop();
    push(putc(__EXTERN__cID_38));
}
fun shift() {  let dy = idx(STACK, add(SP, -0));
  let dx = idx(STACK, add(SP, -1));
  let p = idx(STACK, add(SP, -4));
   let ebp = new_scope();
    pusharr(idx(p, 0), 1);
    pusharr(dx, 1);
    let __EXTERN__YID_46 = pop();
    let __EXTERN__XID_46 = pop();
    push(addi(__EXTERN__XID_46, __EXTERN__YID_46));
    push(idx(p, 0));
    poparr(pop(), 1);
    pusharr(idx(p, 1), 1);
    pusharr(dy, 1);
    let __EXTERN__YID_47 = pop();
    let __EXTERN__XID_47 = pop();
    push(addi(__EXTERN__XID_47, __EXTERN__YID_47));
    push(idx(p, 1));
    poparr(pop(), 1);
    pusharr(p, 3);
    ret(ebp, 3);
}
    push(1);
    push(2);
    test();
    let __EXTERN__nID_48 = pop();
    push(puti(__EXTERN__nID_48));
    push('\n');
    let __EXTERN__cID_49 = pop();
    push(putc(__EXTERN__cID_49));
    pusharr(p, 3);
    test2();
    pusharr(p, 3);
    push(1);
    push(2);
    shift();
    test2();

let static ARRAY = [1, 2, 3];
extern fun puti(i);
extern fun putc(i);
extern fun idx(ptr, i);
extern fun deref(ptr);

puti(deref(idx(ARRAY, 0)));
putc(10);
puti(deref(idx(ARRAY, 1)));
putc(10);
puti(deref(idx(ARRAY, 2)));
putc(10);
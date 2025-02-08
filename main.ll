; ModuleID = 'mage_output'
target triple = "arm64-apple-macosx14.0.0"

declare i64 @_puti(i64)
declare i64 @_putc(i64)
declare i64 @_idx(i64, i64)
declare i64 @_deref(i64)
@_ARRAY = global [3 x i64] zeroinitializer

define i64 @main() {
  %r0 = load i64, i64* @_ARRAY
  %r1 = call i64 @_idx(i64 %r0, i64 0)
  %r2 = call i64 @_deref(i64 %r1)
  %r3 = call i64 @_puti(i64 %r2)
  %r4 = call i64 @_putc(i64 10)
  %r5 = load i64, i64* @_ARRAY
  %r6 = call i64 @_idx(i64 %r5, i64 1)
  %r7 = call i64 @_deref(i64 %r6)
  %r8 = call i64 @_puti(i64 %r7)
  %r9 = call i64 @_putc(i64 10)
  %r10 = load i64, i64* @_ARRAY
  %r11 = call i64 @_idx(i64 %r10, i64 2)
  %r12 = call i64 @_deref(i64 %r11)
  %r13 = call i64 @_puti(i64 %r12)
  %r14 = call i64 @_putc(i64 10)
  ret i64 0
}

; ModuleID = 'examples/loop.ty'
source_filename = "examples/loop.ty"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-f80:128-n8:16:32:64-S128"

declare i32 @puts(i8*)

define i32 @main() {
main.0:
  %i = alloca i32, align 4
  store i32 0, i32* %i, align 4
  br label %main.5

main.5:                                           ; preds = %main.5, %main.0
  store i32 10, i32* %i, align 4
  br label %main.5
}

define void @bar() {
bar.0:
  ret void
}

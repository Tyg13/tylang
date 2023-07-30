; ModuleID = 'examples/fibonacci.ty'
source_filename = "examples/fibonacci.ty"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-f80:128-n8:16:32:64-S128"

define i64 @fibonacci(i64 %0) {
fibonacci.0:
  %lte = icmp sle i64 %0, 1
  br i1 %lte, label %fibonacci.3, label %fibonacci.5

fibonacci.3:                                      ; preds = %fibonacci.0
  ret i32 1
  br label %fibonacci.6

fibonacci.5:                                      ; preds = %fibonacci.0
  br label %fibonacci.6

fibonacci.6:                                      ; preds = %fibonacci.5, %fibonacci.3
  %diff = sub i64 %0, 1
  %call = call i64 @fibonacci(i64 %diff)
  %diff1 = sub i64 %0, 2
  %call2 = call i64 @fibonacci(i64 %diff1)
  %sum = add i64 %call, %call2
  ret i64 %sum
  ret void
}

; ModuleID = 'examples/print_arg.ty'
source_filename = "examples/print_arg.ty"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-f80:128-n8:16:32:64-S128"

@str = private unnamed_addr constant [14 x i8] c"num args: %d\0A\00", align 1

declare void @printf(i8*, ...)

declare void @puts(i8*)

define i32 @main(i32 %argc, i8** %argv) {
main.2:
  %i = alloca i32, align 4
  store i32 0, i32* %i, align 4
  br label %main.7

main.7:                                           ; preds = %main.11, %main.2
  %copy = load i32, i32* %i, align 4
  %cmp = icmp slt i32 %copy, %argc
  br i1 %cmp, label %main.11, label %main.24

main.11:                                          ; preds = %main.7
  %copy1 = load i32, i32* %i, align 4
  %sext = sext i32 %copy1 to i64
  %gep = getelementptr inbounds i8*, i8** %argv, i64 %sext
  %load = load i8*, i8** %gep, align 8
  call void @puts(i8* %load)
  %copy2 = load i32, i32* %i, align 4
  %add = add nsw i32 %copy2, 1
  store i32 %add, i32* %i, align 4
  br label %main.7

main.24:                                          ; preds = %main.7
  br label %main.28

main.28:                                          ; preds = %main.24
  %copy3 = load i32, i32* %i, align 4
  call void (i8*, ...) @printf(i8* getelementptr inbounds ([14 x i8], [14 x i8]* @str, i32 0, i32 0), i32 %copy3)
  ret i32 0
}

# Overview

`tylang` is a compiled, statically typed language using LLVM as its code-generating backend.

It doesn't do much, and only has signed integers so far, but has plans to support a much larger type system with powerful type inference (Hindley-Milner, most likely).

# Building

`tylang` depends on LLVM 15.0, through `inkwell`. `inkwell` itself depends on `llvm-sys`, which relies on an environment variable `LLVM_SYS_<VERSION>_PREFIX` to locate LLVM itself. This means, to build `tylang` you will need to set `LLVM_SYS_150_PREFIX` to the top-level directory where LLVM 15.0 was installed.

# Caveats

This is just a hobby project! It probably will never do anything useful.

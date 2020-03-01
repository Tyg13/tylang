# Overview

`tylang` is a compiled, statically typed language using LLVM as its code-generating backend.

It doesn't do much, and only has one type so far: 64 bit integers, but has plans to support a much larger type system with powerful type inference (Hindley-Milner, most likely).

# Building

`tylang` depends on LLVM 6.0, through `inkwell`. `inkwell` itself depends on `llvm-sys`, which relies on an environment variable `LLVM_SYS_<VERSION>_PREFIX` to locate LLVM itself. This means, to build `tylang` you will need to set `LLVM_SYS_60_PREFIX` to the top-level directory where LLVM 6.0 was installed.

# Caveats

This is just a hobby project! It probably will never do anything useful.

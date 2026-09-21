---
title: Compile-time directives
linkTitle: Compile-time directives
description: Changing rules and behaviors with compile-time directives
weight: 30
---

There are a number of compile-time options exposed in `CMakeLists.txt` in the repo root.
See these for a canonical list. Due to how much history has built up over time, your
best bet is to review the sources to see what these do.

`BTECH_ENABLE_HARDENING` defaults to `ON`. It applies Clang's equivalents of the
C-relevant GCC `-fhardened` protections to first-party MUX and BTech code, and
builds executables with PIE, RELRO, and immediate symbol binding. Set the environment
variable to `OFF` when invoking `just` or pass `-DBTECH_ENABLE_HARDENING=OFF` to CMake
to disable this baseline for a specialized build.

# Lua 4.0 dependency

Official source: https://www.lua.org/ftp/lua-4.0.tar.gz

Archive SHA-256: `b476abc737bf82781cb215e59a259bf23adbdc82425907b51f37ba29c0e71337`.
Copyright belongs to TeCGraf, PUC-Rio; see COPYRIGHT and the source notices.
This is third-party interpreter code, not reconstructed Monopoly Tycoon code.

Only core C sources and public headers are included. No standard libraries or
command-line interpreter are built. Script buffers and game calls come from Rust.

Marked OpenMT2001 alteration in `src/llimits.h`: `Instruction` and `lint32` use
`uint32_t`, and pointer hashing uses `uintptr_t`, preserving 32-bit instruction
width on LP64. Pointer hashes use the low 32 bits after shifting a `uintptr_t`.
`lobject.h`, `lstring.c`, and `ltable.c` keep string/hash arithmetic explicitly
32-bit across targets; these changes are also marked in the source.
Numeric hashes in `ltable.c` call the allocation-free Rust FTOL helper, matching
the original executable's `00508c20`/`00508c80` conversion through `0050e928`.

Native compilation is behind the optional `lua4` feature. A browser scripting
runtime still requires a C toolchain/runtime with working setjmp/longjmp, such as
Emscripten; the ordinary wasm32 graphics/simulation library remains independent.

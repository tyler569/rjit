
# RJIT

This project implements a simple assembler/JIT skeleton in Rust.

Example:

```rust
    let mut c = Code::new();

    c.mov_imm32(RAX, 1);
    c.mov_imm32(RBX, 1);

    let label = c.here();

    c.add_r(RBX, RAX);
    c.add_r(RAX, RBX);

    c.cmp_imm32(RAX, 1000);

    let offset = label - c.here();
    c.cjmp(Jng, offset as i32);

    c.ret();


    let exec =
        MemoryMap::new(0x1000, &[MapReadable, MapWritable, MapExecutable])
            .unwrap();

    let fibonacci = unsafe {
        ptr::copy(c.buf.as_ptr(), exec.data(), c.buf.len());
        transmute::<_, fn() -> usize>(exec.data())
    };

    
    let result = fibonacci();
```

The methods on the `Code` object emit the bytes corresponding to those assembly instructions into a buffer, which is then mapped executable and run.

## TODO

- Write docuementation
- More addressing modes (indirect)
- Support R8-R15 registers (REX\_B and REX\_R)
- Higher level constructs
    - Functions (prelude and locals)
    - "Locals" (variables accessed indirectly through base pointer)

// temporary
#![allow(dead_code)]

extern crate mmap;

use std::mem::transmute;
use std::ptr;

use mmap::{MapOption, MemoryMap};
use MapOption::*;

fn imm_bits(val: i32) -> i32 {
    if -0x80 < val && val < 0x80 {
        8
    } else {
        32
    }
}

const REX_W: u8 = 0x48;

const RAX: u8 = 0;
const RCX: u8 = 1;
const RDX: u8 = 2;
const RBX: u8 = 3;
const RSP: u8 = 4;
const RBP: u8 = 5;
const RSI: u8 = 6;
const RDI: u8 = 7;

enum JumpType {
    Jo = 0x70,  //             OF=1
    Jno = 0x71, //             OF=0
    Jb = 0x72,  // jnae jc     CF=1
    Jnb = 0x73, // jna  jnc    CF=0
    Je = 0x74,  // jz          ZF=1
    Jne = 0x75, // jnz         ZF=0
    Jna = 0x76, // jbe         CF=1 OR ZF=1
    Ja = 0x77,  // jnbe        CF=0 AND ZF=0
    Js = 0x78,  //             SF=1
    Jns = 0x79, //             SF=0
    Jp = 0x7A,  // jpe         PF=1
    Jnp = 0x7B, // jpo         PF=0
    Jl = 0x7C,  // jnge        SF!=OF
    Jnl = 0x7D, // jge         SF==OF
    Jng = 0x7E, // jle         ZF=1 OR SF!=OF
    Jg = 0x7F,  // jnle        ZF=0 AND SF==OF
}

fn mod_rm(mod_b: u8, r2: u8, r1: u8) -> u8 {
    (mod_b << 6) + (r2 << 3) + (r1)
}

use JumpType::*;

#[derive(Debug)]
struct Code {
    buf: Vec<u8>,
}

impl Code {
    fn new() -> Code {
        Code { buf: vec![] }
    }

    fn print_hex(&self) {
        for byte in self.buf.iter() {
            print!("{:02x} ", byte);
        }
        println!();
    }

    fn emit(&mut self, data: &[u8]) {
        for byte in data {
            self.buf.push(*byte);
        }
    }

    fn emit_i32(&mut self, val: i32) {
        let bytes: [u8; 4] = unsafe { transmute(val.to_le()) };
        for byte in &bytes {
            self.buf.push(*byte);
        }
    }

    fn mov_imm32(&mut self, reg: u8, val: i32) {
        self.emit(&[0xB8 + reg]);
        self.emit_i32(val);
    }

    fn jmp(&mut self, target: i32) {
        /*
         * The target - x is because intel jmp targets are relative to the
         * *next* instruction after the jmp.  This is weird and means you
         * have to know the distance before you can decide which offset to use.
         * I solve this problem by making it transparent and taking an offset
         * relative to the actual jmp instruction and correct for it manually.
         */
        match imm_bits(target) {
            8 => {
                self.emit(&[0xEB, (target - 2) as u8]);
            }
            32 => {
                self.emit(&[0xE9]);
                self.emit_i32(target - 5);
            }
            _ => panic!("oops"),
        }
    }

    fn call_rel32(&mut self, target: i32) {
        self.emit(&[0xE8]);
        self.emit_i32(target - 5);
    }

    fn ret(&mut self) {
        self.emit(&[0xC3]);
    }

    fn cjmp(&mut self, t: JumpType, target: i32) {
        match imm_bits(target) {
            8 => {
                self.emit(&[t as u8, (target - 2) as u8]);
            }
            32 => {
                self.emit(&[0x0F, t as u8 + 0x10]);
                self.emit_i32(target - 6);
            }
            _ => panic!("oops"),
        }
    }

    fn add_r(&mut self, r_dst: u8, r_src: u8) {
        self.emit(&[REX_W, 0x01, mod_rm(3, r_dst, r_src)]);
    }

    fn cmp_r(&mut self, r2: u8, r1: u8) {
        self.emit(&[REX_W, 0x39, mod_rm(3, r2, r1)]);
    }

    fn cmp_imm32(&mut self, r: u8, imm: i32) {
        match imm_bits(imm) {
            8 => {
                self.emit(&[REX_W, 0x83, mod_rm(3, 7, r), imm as u8]);
            }
            32 => {
                self.emit(&[REX_W, 0x81, mod_rm(3, 7, r)]);
                self.emit_i32(imm);
            }
            _ => panic!("oops"),
        }
    }

    fn here(&self) -> isize {
        self.buf.len() as isize
    }
}

fn main() {
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

    c.print_hex();

    let exec =
        MemoryMap::new(0x1000, &[MapReadable, MapWritable, MapExecutable])
            .unwrap();

    let func = unsafe {
        ptr::copy(c.buf.as_ptr(), exec.data(), c.buf.len());
        transmute::<_, fn() -> usize>(exec.data())
    };

    println!("calling buffer returned: {}", func());
}

#[test]
fn test_basic_emit_code() {
    let mut c = Code::new();
    c.emit(&[0x01]);
    c.emit_i32(1);

    assert!(c.buf == vec![1, 1, 0, 0, 0]);
}

#[test]
fn test_emit_mov_imm32_1() {
    let mut c = Code::new();
    c.mov_imm32(RAX, 1);

    assert!(c.buf == vec![0xB8, 1, 0, 0, 0]);
}

#[test]
fn test_emit_mov_imm32_minus_1() {
    let mut c = Code::new();
    c.mov_imm32(RDI, -1);

    assert!(c.buf == vec![0xBF, 255, 255, 255, 255]);
}

#[test]
fn test_jmp_short_forward() {
    let mut c = Code::new();
    c.jmp(10);

    assert!(c.buf == vec![0xEB, 8]);
}

#[test]
fn test_jmp_short_backward() {
    let mut c = Code::new();
    c.jmp(-10);

    assert!(c.buf == vec![0xEB, 0xF4]);
}

#[test]
fn test_jmp_long_forward() {
    let mut c = Code::new();
    c.jmp(0x110);

    c.print_hex();
    assert!(c.buf == vec![0xE9, 0x0B, 0x01, 0x00, 0x00]);
}

#[test]
fn test_jmp_long_backward() {
    let mut c = Code::new();
    c.jmp(-1000);

    c.print_hex();
    assert!(c.buf == vec![0xE9, 0x13, 0xFC, 0xFF, 0xFF]);
}

#[test]
fn test_call_rel32() {
    let mut c = Code::new();
    c.call_rel32(0x110);

    c.print_hex();
    assert!(c.buf == vec![0xE8, 0x0B, 0x01, 0x00, 0x00]);
}

#[test]
fn test_ret() {
    let mut c = Code::new();
    c.ret();

    c.print_hex();
    assert!(c.buf == vec![0xC3]);
}

#[test]
fn test_cjmp_short() {
    let mut c = Code::new();
    c.cjmp(Jo, 10);

    c.print_hex();
    assert!(c.buf == vec![0x70, 8]);
}

#[test]
fn test_cjmp_long() {
    let mut c = Code::new();
    c.cjmp(Jo, 0x110);

    c.print_hex();
    assert!(c.buf == vec![0x0F, 0x80, 0x0A, 0x01, 0x00, 0x00]);
}

#[test]
fn test_add_r() {
    let mut c = Code::new();
    c.add_r(RAX, RAX);

    c.print_hex();
    assert!(c.buf == vec![0x48, 0x01, 0xC0]);
}

#[test]
fn test_cmp_r() {
    let mut c = Code::new();
    c.cmp_r(RAX, RAX);

    c.print_hex();
    assert!(c.buf == vec![0x48, 0x39, 0xC0]);
}

#[test]
fn test_cmp_imm32() {
    let mut c = Code::new();
    c.cmp_imm32(RAX, 0x1000);

    c.print_hex();
    assert!(c.buf == vec![0x48, 0x81, 0xF8, 0x00, 0x10, 0x00, 0x00]);
}

#[test]
fn test_cmp_imm32_short() {
    let mut c = Code::new();
    c.cmp_imm32(RAX, 0x01);

    c.print_hex();
    assert!(c.buf == vec![0x48, 0x83, 0xF8, 0x01]);
}

#[test]
fn test_emit_fib() {
    let mut c = Code::new();

    let wanted_code = vec![
        0xb8, 0x01, 0x00, 0x00, 0x00, // mov    eax,0x1
        0xbb, 0x01, 0x00, 0x00, 0x00, // mov    ebx,0x1
        0x48, 0x01, 0xd8, // add    rax,rbx
        0x48, 0x01, 0xc3, // add    rbx,rax
        0x48, 0x81, 0xf8, 0xe8, 0x03, 0x00, 0x00, // cmp    rax,0x3e8
        0x7e, 0xf1, // jle    0xa
        0xc3, // ret
    ];

    c.mov_imm32(RAX, 1);
    c.mov_imm32(RBX, 1);

    let label = c.here();

    c.add_r(RBX, RAX);
    c.add_r(RAX, RBX);

    c.cmp_imm32(RAX, 1000);

    let offset = label - c.here();
    c.cjmp(Jng, offset as i32);

    c.ret();

    c.print_hex();

    assert!(c.buf == wanted_code);
}

#[test]
fn test_execute_buffer() {
    let mut c = Code::new();

    c.mov_imm32(RAX, 10);
    c.ret();

    let exec =
        MemoryMap::new(c.buf.len(), &[MapReadable, MapWritable, MapExecutable])
            .unwrap();

    let func = unsafe {
        ptr::copy(c.buf.as_ptr(), exec.data(), c.buf.len());
        transmute::<_, fn() -> usize>(exec.data())
    };

    assert!(func() == 10);
}


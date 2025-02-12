use mage::Interface;
use anyhow::Result;
use std::{io::Read, collections::HashMap};
use tracing::*;
use lazy_static::lazy_static;

#[derive(Default)]
pub struct cloverInterface;

lazy_static! {
    static ref BUILTINS: HashMap<&'static str, fn(Vec<i64>) -> i64> = {
        let mut map: HashMap<&'static str, fn(Vec<i64>) -> i64> = HashMap::new();

        map.insert("read", |_args| {
            // Read in exactly one character
            let mut buf = [0; 1];
            let _ = std::io::stdin().read_exact(&mut buf);
            buf[0] as i64
        });

        map.insert("write", |args| {
            print!("{}", args[0]);
            0
        });

        map.insert("putc", |args| {
            print!("{}", args[0] as u8 as char);
            0
        });
        map.insert("putchar", |args| {
            print!("{}", args[0] as u8 as char);
            0
        });
        map.insert("putln", |_| {
            println!();
            0
        });

        map.insert("puti", |args| {
            // ti.putint(args[0]);
            print!("{}", args[0]);
            0
        });
        map.insert("putint", |args| {
            // ti.putint(args[0]);
            print!("{}", args[0]);
            0
        });
        map.insert("putp", |args| {
            // ti.putint(args[0]);
            print!("{:p}", args[0] as *const i64);
            0
        });


        map.insert("putf", |args| {
            print!("{}", f64::from_bits(args[0] as u64));
            0
        });


        map.insert("putcstr", |args| {
            let s = unsafe { std::ffi::CStr::from_ptr(args[0] as *const i8) };
            print!("{}", s.to_str().unwrap());
            0
        });
        map.insert("putstr", |args| {
            // let s = unsafe { std::ffi::CStr::from_ptr(args[0] as *const i64) };
            unsafe {
                let s = args[0] as *const i64;
                let mut i = 0;
                while *s.add(i) != 0 {
                    print!("{}", *s.add(i) as u8 as char);
                    i += 1;
                }
            }
            0
        });
        
        map.insert("putsln", |args| {
            let s = unsafe { std::ffi::CStr::from_ptr(args[0] as *const i8) };
            println!("{}", s.to_str().unwrap());
            0
        });

        map.insert("clover_idx", |args| {
            let ptr = args[0] as *const i64;
            let idx = args[1] as i64;
            unsafe {
                let result = ptr.add(idx as usize) as i64;
                debug!("Index {} + {} = {}", ptr as usize, idx, result);
                result
            }
        });
        map.insert("clover_memcpy", |args| {
            let dst = args[0] as *mut i64;
            let src = args[1] as *const i64;
            let size = args[2] as u64;
            debug!("Copying {} bytes from {} to {}", size * 8, src as usize, dst as usize);
            unsafe {
                std::ptr::copy(src, dst, size as usize * 8);
            }
            0
        });

        map.insert("malloc", |args| {
            let size = args[0] as usize;
            // Call malloc
            let ptr = Box::into_raw(vec![0; size * 8 + 100].into_boxed_slice());
            ptr as *const i64 as i64
        });

        map.insert("clover_malloc", |args| {
            let size = args[0] as usize;
            // Call malloc
            let ptr = Box::into_raw(vec![0; size * 8 + 100].into_boxed_slice());
            let result = (ptr as *const i64) as i64;
            result
        });

        map.insert("free", |args| {
            let ptr = args[0] as *mut i64;
            let _ = unsafe { Box::from_raw(ptr) };
            0
        });

        map.insert("clover_deref", |args| {
            let ptr = args[0] as *const i64;
            debug!("Dereffing {}", ptr as usize);
            if args[0] == 0 {
                error!("Dereffing null");
                return 0;
            }
            unsafe {
                *ptr
            }
        });

        map.insert("fadd", |args| f64::to_bits(f64::from_bits(args[0] as u64) + f64::from_bits(args[1] as u64)) as i64);
        map.insert("fsub", |args| f64::to_bits(f64::from_bits(args[0] as u64) - f64::from_bits(args[1] as u64)) as i64);
        map.insert("fmul", |args| f64::to_bits(f64::from_bits(args[0] as u64) * f64::from_bits(args[1] as u64)) as i64);
        map.insert("fdiv", |args| f64::to_bits(f64::from_bits(args[0] as u64) / f64::from_bits(args[1] as u64)) as i64);
        map.insert("fneg", |args| f64::to_bits(-f64::from_bits(args[0] as u64)) as i64);
        map.insert("frem", |args| f64::to_bits(f64::from_bits(args[0] as u64) % f64::from_bits(args[1] as u64)) as i64);

        map.insert("flt", |args| if f64::from_bits(args[0] as u64) < f64::from_bits(args[1] as u64) { 1 } else { 0 });
        map.insert("lt", |args| if args[0] < args[1] { 1 } else { 0 });
        map.insert("le", |args| if args[0] <= args[1] { 1 } else { 0 });

        map.insert("add", |args| args[0] + args[1]);
        map.insert("clover_add", |args| args[0] + args[1]);
        map.insert("sub", |args| args[0] - args[1]);
        map.insert("clover_sub", |args| args[0] - args[1]);
        map.insert("div", |args| args[0] / args[1]);
        map.insert("mul", |args| args[0] * args[1]);
        map.insert("clover_mul", |args| args[0] * args[1]);
        map.insert("rem", |args| args[0] % args[1]);

        map.insert("eq", |args| if args[0] == args[1] { 1 } else { 0 });
        map.insert("neq", |args| if args[0] != args[1] { 1 } else { 0 });

        map.insert("round", |args| f64::to_bits(f64::from_bits(args[0] as u64).round()) as i64);
        map.insert("floor", |args| f64::to_bits(f64::from_bits(args[0] as u64).floor()) as i64);
        map.insert("ceil", |args| f64::to_bits(f64::from_bits(args[0] as u64).ceil()) as i64);

        map.insert("to_float", |args| f64::to_bits(args[0] as f64) as i64);
        map.insert("to_int", |args| f64::from_bits(args[0] as u64) as i64);
        
        map
    };
}

impl Interface for cloverInterface {
    fn has_extern(&self, name: &str) -> bool {
        BUILTINS.contains_key(name)
    }

    fn external_call(&mut self, name: &str, args: Vec<i64>) -> Result<i64> {
        use tracing::*;
        debug!("Calling builtin {}", name);
        Ok(BUILTINS[name](args))
    }
}
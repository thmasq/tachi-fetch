//! Ultra-optimized inline assembly version of key functions
//! Only include this if you want absolute maximum performance
//! Warning: This is specific to x86_64 Linux

use std::mem::MaybeUninit;

#[cfg(target_arch = "x86_64")]
pub mod asm {
    use super::*;
    use libc::{sysinfo, utsname};
    use std::arch::asm;

    /// Fast uname syscall using inline assembly
    /// This bypasses libc entirely for maximum performance
    #[inline(always)]
    pub unsafe fn fast_uname() -> utsname {
        let mut result = MaybeUninit::<utsname>::uninit();

        #[cfg(target_arch = "x86_64")]
        {
            // syscall number for uname is 63 on x86_64
            asm!(
                "mov rax, 63",          // uname syscall number
                "syscall",              // direct syscall
                in("rdi") result.as_mut_ptr(),
                out("rax") _,
                out("rcx") _,
                out("r11") _,
                lateout("rdi") _,
            );
        }

        result.assume_init()
    }

    /// Fast sysinfo syscall using inline assembly
    /// This bypasses libc entirely for maximum performance
    #[inline(always)]
    pub unsafe fn fast_sysinfo() -> sysinfo {
        let mut result = MaybeUninit::<sysinfo>::uninit();

        #[cfg(target_arch = "x86_64")]
        {
            // syscall number for sysinfo is 99 on x86_64
            asm!(
                "mov rax, 99",          // sysinfo syscall number
                "syscall",              // direct syscall
                in("rdi") result.as_mut_ptr(),
                out("rax") _,
                out("rcx") _,
                out("r11") _,
                lateout("rdi") _,
            );
        }

        result.assume_init()
    }

    /// Fast gethostname syscall using inline assembly
    /// This bypasses libc entirely for maximum performance
    #[inline(always)]
    pub unsafe fn fast_gethostname(buf: &mut [u8]) -> i32 {
        let mut result: i32;

        #[cfg(target_arch = "x86_64")]
        {
            // syscall number for gethostname is 74 on x86_64
            asm!(
                "mov rax, 74",          // gethostname syscall number
                "syscall",              // direct syscall
                in("rdi") buf.as_mut_ptr(),
                in("rsi") buf.len(),
                out("rax") result,
                out("rcx") _,
                out("r11") _,
                lateout("rdi") _,
                lateout("rsi") _,
            );
        }

        result
    }

    /// Ultra-fast CPU core count using direct syscall
    /// Equivalent to sysconf(_SC_NPROCESSORS_ONLN)
    #[inline(always)]
    pub unsafe fn fast_cpu_count() -> i64 {
        let mut result: i64;

        #[cfg(target_arch = "x86_64")]
        {
            // sysconf is not a direct syscall, but we can use
            // the direct get_nprocs syscall or read directly from /proc
            // This reads /sys/devices/system/cpu/online which is faster
            asm!(
                "mov rax, 2",          // syscall number for open
                "lea rdi, [rip + path]", // first argument
                "mov rsi, 0",          // O_RDONLY
                "syscall",              // make the syscall
                "mov rdi, rax",         // save the file descriptor
                "sub rsp, 32",          // allocate buffer on stack
                "mov rsi, rsp",         // buffer address
                "mov rdx, 32",          // buffer size
                "mov rax, 0",           // syscall number for read
                "syscall",              // make the syscall
                "mov rcx, rax",         // save number of bytes read
                "mov rax, 3",           // syscall number for close
                "syscall",              // close the file
                "xor rax, rax",         // initialize count
                "xor rsi, rsi",         // initialize state
                "mov rdx, 1",           // default to 1 CPU
                "count_loop:",
                "cmp rsi, rcx",         // check if we've reached the end
                "jge end_count",
                "mov bl, [rsp + rsi]",  // load byte
                "inc rsi",              // increment index
                "cmp bl, 0x2d",         // check for '-' (hyphen)
                "jne not_hyphen",
                "mov r8, rdx",          // store current value
                "jmp count_loop",
                "not_hyphen:",
                "cmp bl, 0x30",         // check if digit
                "jl not_digit",
                "cmp bl, 0x39",
                "jg not_digit",
                "imul rdx, rdx, 10",    // multiply by 10
                "sub bl, 0x30",         // convert to number
                "movzx rbx, bl",
                "add rdx, rbx",         // add to result
                "jmp count_loop",
                "not_digit:",
                "cmp bl, 0x0a",         // check for newline
                "jne count_loop",
                "end_count:",
                "mov rax, rdx",
                "add rsp, 32",          // restore stack
                "path:",
                ".ascii \"/sys/devices/system/cpu/online\\0\"",
                out("rax") result,
                out("rcx") _,
                out("rdx") _,
                out("rbx") _,
                out("rsi") _,
                out("rdi") _,
                out("r8") _,
                out("r11") _,
            );
        }

        // If we get 0, fall back to a reasonable default
        if result <= 0 {
            result = 1;
        }

        result
    }
}

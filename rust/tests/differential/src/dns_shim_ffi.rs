// FFI bindings for dns_shim.c — standalone C DNS cache for differential tests.

use std::os::raw::{c_char, c_int, c_void};

#[repr(C)]
pub struct CDnsShim {
    _opaque: [u8; 0],
}

extern "C" {
    pub fn dns_shim_new(net: u32, mask: u32, max: c_int) -> *mut CDnsShim;
    pub fn dns_shim_free(shim: *mut CDnsShim);
    pub fn dns_shim_handle(
        shim: *mut CDnsShim,
        req: *mut c_void,
        qlen: c_int,
        res: *mut c_void,
        slen: c_int,
    ) -> c_int;
    pub fn dns_shim_lookup(shim: *mut CDnsShim, ip: u32) -> *const c_char;
}

/// Safe RAII wrapper around the C DnsShim.
pub struct CDnsShimWrapper {
    ptr: *mut CDnsShim,
}

impl CDnsShimWrapper {
    pub fn new(net: u32, mask: u32, max: usize) -> Self {
        let ptr = unsafe { dns_shim_new(net, mask, max as c_int) };
        assert!(!ptr.is_null(), "dns_shim_new returned NULL");
        Self { ptr }
    }

    /// Process a DNS query. Returns response length or -1 on error.
    pub fn handle(&mut self, req: &mut [u8], res: &mut [u8]) -> c_int {
        unsafe {
            dns_shim_handle(
                self.ptr,
                req.as_mut_ptr() as *mut c_void,
                req.len() as c_int,
                res.as_mut_ptr() as *mut c_void,
                res.len() as c_int,
            )
        }
    }

    /// Reverse lookup: ip → name.
    pub fn lookup(&self, ip: u32) -> Option<String> {
        let ptr = unsafe { dns_shim_lookup(self.ptr, ip) };
        if ptr.is_null() {
            None
        } else {
            let cstr = unsafe { std::ffi::CStr::from_ptr(ptr) };
            Some(cstr.to_string_lossy().into_owned())
        }
    }
}

impl Drop for CDnsShimWrapper {
    fn drop(&mut self) {
        unsafe { dns_shim_free(self.ptr) };
    }
}

use ::core::ffi::CStr;
use ::libc::{
	c_void,
	RTLD_NOLOAD,
	dlopen, dlsym, dlclose,
};

use super::UnixObject;

#[derive(Debug)]
pub struct Library {
	base_addr: usize,
	symbols: Symbols,
}

impl Library {
	pub const fn new(object: &UnixObject, symbols: Symbols) -> Self {
		Self {
			base_addr: object.base_addr(),
			symbols,
		}
	}
}
impl super::super::LibraryImpl for Library {
	fn base_addr(&self) -> usize {
		self.base_addr
	}
	fn symbol(&self, name: &CStr) -> *mut () {
		self.symbols.symbol(name) as _
	}
}

#[derive(Debug)]
pub struct Symbols {
	handle: *mut c_void,
}
impl Symbols {
	pub fn open(filename: &CStr) -> Option<Self> {
		unsafe {
			let handle = dlopen(filename.as_ptr(), RTLD_NOLOAD);
			if !handle.is_null() {
				Some(Self {
					handle,
				})
			} else {
				None
			}
		}
	}

	pub fn symbol(&self, name: &CStr) -> *mut c_void {
		unsafe { dlsym(self.handle, name.as_ptr()) }
	}
}
impl Drop for Symbols {
	fn drop(&mut self) {
		unsafe { dlclose(self.handle) };
	}
}

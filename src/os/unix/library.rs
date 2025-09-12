use ::libc::{
	c_void,
	RTLD_LAZY, RTLD_NOLOAD,
	dlopen, dlsym, dlclose,
	dlerror,
};
use ::std::{
	error::Error as StdError,
	ffi::{
		CStr, CString,
	},
	fmt::{
		self, Write,
	},
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
	pub fn open(filename: &CStr) -> Result<Self, Error> {
		unsafe {
			let handle = dlopen(filename.as_ptr(), RTLD_LAZY | RTLD_NOLOAD);
			if !handle.is_null() {
				Ok(Self {
					handle,
				})
			} else {
				Err(Error::last_error())
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

#[derive(Debug)]
#[repr(transparent)]
pub struct Error(CString);
impl Error {
	pub fn last_error() -> Self {
		let message = unsafe {
			let message_ptr = dlerror();
			if !message_ptr.is_null() {
				CStr::from_ptr(message_ptr)
			} else {
				c"(no information available)"
			}
		};
		Self::from_c_str(message)
	}

	fn from_c_str(s: &CStr) -> Self {
		Self(CString::from(s))
	}
}
impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		for chunk in self.0.as_bytes().utf8_chunks() {
			f.write_str(chunk.valid())?;
			for _ in chunk.invalid() {
				f.write_char(char::REPLACEMENT_CHARACTER)?;
			}
		}
		Ok(())
	}
}
impl StdError for Error {}

use ::core::{
	ffi::CStr,
	mem::MaybeUninit,
};
use ::winapi::{
	shared::minwindef::{
		HMODULE, FARPROC,
		FALSE,
	},
	um::libloaderapi::{
		GetModuleHandleExA, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
		GetProcAddress, FreeLibrary,
	},
};

use super::{
	Module, Error,
};

pub(crate) type Library = OwnedModule;
impl super::super::LibraryImpl for Library {
	fn base_addr(&self) -> usize {
		self.0 as _
	}
	fn symbol(&self, name: &CStr) -> *mut () {
		OwnedModule::symbol(self, name) as _
	}
}

#[derive(Debug)]
#[repr(transparent)]
pub struct OwnedModule(HMODULE);
impl OwnedModule {
	pub fn from_module(module: &Module) -> Result<Self, Error> {
		unsafe {
			let mut handle = MaybeUninit::<HMODULE>::uninit();
			let is_ok = GetModuleHandleExA(
				GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, module.handle as _,
				handle.as_mut_ptr(),
			);
			if is_ok == FALSE {
				return Err(Error::last_os_error())
			}
			Ok(Self(handle.assume_init()))
		}
	}

	pub const fn base_ptr(&self) -> *mut () {
		self.0 as _
	}

	pub fn symbol(&self, name: &CStr) -> FARPROC {
		unsafe { GetProcAddress(self.0, name.as_ptr()) }
	}
}
impl Drop for OwnedModule {
	fn drop(&mut self) {
		unsafe { FreeLibrary(self.0) };
	}
}

use ::core::{
	ffi::CStr,
	mem::{
		MaybeUninit, size_of_val,
	},
};
use ::winapi::{
	shared::minwindef::FALSE,
	um::{
		handleapi::{
			CloseHandle,
			INVALID_HANDLE_VALUE,
		},
		processthreadsapi::GetCurrentProcessId,
		tlhelp32::{
			CreateToolhelp32Snapshot,
			Module32First, Module32Next,
			MODULEENTRY32, MAX_MODULE_NAME32,
			TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32,
		},
		winnt::{
			CHAR, HANDLE,
		},
	},
};

use super::{
	Error, Module,
};

pub struct ModuleSnapshot {
	handle: OwnedHandle,
	first_entry: MODULEENTRY32,
}
impl ModuleSnapshot {
	pub fn new() -> Result<Self, Error> {
		unsafe {
			let handle = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, GetCurrentProcessId());
			let handle = OwnedHandle::new(handle).ok_or_else(Error::last_os_error)?;

			let mut entry = MaybeUninit::<MODULEENTRY32>::zeroed().assume_init();
			entry.dwSize = size_of_val(&entry) as _;
			if Module32First(handle.get(), &mut entry) == 0 {
				return Err(Error::last_os_error())
			}

			Ok(Self {
				handle,
				first_entry: entry,
			})
		}
	}

	pub const fn iter(&self) -> Modules<'_> {
		Modules {
			snapshot: self,
			entry: Some(self.first_entry),
		}
	}
}

pub struct Modules<'a> {
	snapshot: &'a ModuleSnapshot,
	entry: Option<MODULEENTRY32>,
}
impl Iterator for Modules<'_> {
	type Item = (OwnedModuleName, Module);
	fn next(&mut self) -> Option<Self::Item> {
		unsafe {
			let entry = self.entry.as_mut()?;
			let module = Module {
				handle: entry.hModule,
				size: entry.modBaseSize,
			};
			let name = OwnedModuleName::new(entry.szModule);
			let result = Module32Next(self.snapshot.handle.get(), entry);
			if result == FALSE {
				self.entry = None;
			}
			Some((name, module))
		}
	}
}

#[repr(transparent)]
pub struct OwnedModuleName {
	// INVARIANT: The buffer always contains a valid C string.
	buffer: [CHAR; MAX_MODULE_NAME32 + 1],
}

impl OwnedModuleName {
	/// # Safety
	/// `buffer` must contain a valid C string.
	pub const unsafe fn new(buffer: [CHAR; MAX_MODULE_NAME32 + 1]) -> Self {
		Self {
			buffer,
		}
	}

	pub const fn as_c_str(&self) -> &CStr {
		unsafe { CStr::from_ptr(self.buffer.as_ptr()) }
	}
}

impl AsRef<CStr> for OwnedModuleName {
	fn as_ref(&self) -> &CStr {
		self.as_c_str()
	}
}

#[repr(transparent)]
struct OwnedHandle(HANDLE);
impl OwnedHandle {
	pub unsafe fn new(inner: HANDLE) -> Option<Self> {
		if inner != INVALID_HANDLE_VALUE {
			Some(Self(inner))
		} else {
			None
		}
	}

	pub const fn get(&self) -> HANDLE {
		self.0
	}
}
impl Drop for OwnedHandle {
	fn drop(&mut self) {
		unsafe { CloseHandle(self.0) };
	}
}

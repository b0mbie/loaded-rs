use ::core::{
	ffi::CStr,
	mem::{
		MaybeUninit, size_of_val,
	},
};
use ::winapi::{
	shared::minwindef::{
		DWORD, HMODULE, FARPROC,
		FALSE,
	},
	um::{
		libloaderapi::{
			GetModuleHandleA, GetProcAddress,
		},
		processthreadsapi::GetCurrentProcess,
		psapi::{
			GetModuleInformation, MODULEINFO,
		},
		winnt::IMAGE_DOS_HEADER,
	}
};

use crate::map::*;

use super::lifetime_wrapper;

mod library;
pub use library::*;
mod tlhelp32;
pub use tlhelp32::*;

pub use ::std::io::Error;

lifetime_wrapper! {
	pub(crate) struct Segment for Module;
}

pub(crate) type Segments<'a> = ::core::iter::Once<Segment<'a>>;

lifetime_wrapper! {
	pub(crate) struct ModuleName for OwnedModuleName;
}
impl AsRef<CStr> for ModuleName<'_> {
	fn as_ref(&self) -> &CStr {
		self.inner.as_c_str()
	}
}

#[derive(Debug)]
pub struct Module {
	handle: HMODULE,
	size: DWORD,
}
impl Module {
	pub fn find(name: &CStr) -> Result<Option<Self>, Error> {
		let handle = unsafe { GetModuleHandleA(name.as_ptr()) };
		if !handle.is_null() {
			let found = unsafe {
				let mut info = MaybeUninit::<MODULEINFO>::uninit();
				let is_ok = GetModuleInformation(GetCurrentProcess(), handle, info.as_mut_ptr(), size_of_val(&info) as _);
				if is_ok == FALSE {
					return Err(Error::last_os_error())
				}

				let info = info.assume_init();
				Self {
					// base_ptr: info.lpBaseOfDll as _,
					handle,
					size: info.SizeOfImage,
				}
			};
			Ok(Some(found))
		} else {
			Ok(None)
		}
	}

	pub const fn base_ptr(&self) -> *mut u8 {
		self.handle as _
	}

	pub const fn size(&self) -> usize {
		self.size as _
	}

	pub fn symbol(&self, name: &CStr) -> FARPROC {
		unsafe { GetProcAddress(self.handle, name.as_ptr()) }
	}
}

#[derive(Debug)]
pub(crate) struct Symbols;

lifetime_wrapper! {
	#[derive(Debug)]
	pub(crate) struct Object for Module;
}
impl super::ObjectImpl for Object<'_> {
	fn is_main_program(&self) -> bool {
		unsafe extern "C" {
			static __ImageBase: IMAGE_DOS_HEADER;
		}
		unsafe { self.inner.handle == (&__ImageBase as *const _ as _) }
	}
	fn base_addr(&self) -> usize {
		self.inner.handle as _
	}
	fn segments(&self) -> Segments<'_> {
		let segment = Segment::new(Module {
			handle: self.inner.handle,
			size: self.inner.size,
		});
		::core::iter::once(segment)
	}
	fn symbols(&self) -> Symbols {
		Symbols
	}
	fn symbol(&self, symbols: &Symbols, name: &CStr) -> *mut () {
		let _ = symbols;
		self.inner.symbol(name) as _
	}
	fn library(&self, symbols: Symbols) -> Library {
		let _ = symbols;
		match Library::from_module(&self.inner) {
			Ok(lib) => lib,
			Err(error) => {
				panic!("couldn't create owned Windows module:\n{error:#?}");
			}
		}
	}
}

impl super::SegmentImpl for Segment<'_> {
	fn flags(&self) -> SegmentFlags {
		SegmentFlags
	}
	fn virtual_addr(&self) -> usize {
		0
	}
	fn size(&self) -> usize {
		self.inner.size as _
	}
}

#[derive(Clone, Copy)]
#[repr(transparent)]
#[non_exhaustive]
pub struct SegmentFlags;
impl super::SegmentFlagsImpl for SegmentFlags {
	fn has_x(&self) -> bool {
		true
	}
	fn has_r(&self) -> bool {
		true
	}
	fn has_w(&self) -> bool {
		false
	}

	fn is_rx(&self) -> bool {
		true
	}
}

#[derive(Default)]
#[non_exhaustive]
#[repr(transparent)]
pub struct Objects;
impl super::ObjectsImpl for Objects {
	fn init() -> Self {
		Self::new()
	}
	fn find_map<R, F: FnMut(ModuleName<'_>, Object<'_>) -> Option<R>>(&self, mut f: F) -> Result<Option<R>, Error> {
		let snapshot = ModuleSnapshot::new()?;
		let result = snapshot.iter()
			.find_map(move |(name, module)| f(ModuleName::new(name), Object::new(module)));
		Ok(result)
	}
	fn fill_map<'a, M: ?Sized + ObjectMap<'a>>(&self, map: &mut M) -> Result<(), Error> {
		Objects::fill_map(self, map)
	}
	fn map_by_name<R, F: FnOnce(Object<'_>) -> R>(&self, name: &CStr, f: F) -> Result<Option<R>, Error> {
		match self.find_object(name) {
			Ok(Some(module)) => Ok(Some(f(module))),
			Ok(None) => Ok(None),
			Err(error) => Err(error),
		}
	}
	fn for_each<F: FnMut(ModuleName<'_>, Object<'_>) -> bool>(&self, mut f: F) -> Result<(), self::Error> {
		let snapshot = ModuleSnapshot::new()?;
		for (name, module) in snapshot.iter() {
			if f(ModuleName::new(name), Object::new(module)) {
				break
			}
		}
		Ok(())
	}
}

impl Objects {
	pub const fn new() -> Self {
		Self
	}

	pub fn fill_map<'a, M: ?Sized + ObjectMap<'a>>(&self, map: &mut M) -> Result<(), Error> {
		for mut entry in map.entries_mut() {
			let mut found = None;
			for name in entry.names() {
				found = self.find_object(name)?;
				if found.is_some() {
					break
				}
			}
			if let Some(object) = found {
				entry.write(crate::Object(object));
			}
		}
		Ok(())
	}

	pub fn map_by_name<R, F: FnOnce(Module) -> R>(&self, name: &CStr, f: F) -> Result<Option<R>, Error> {
		match Module::find(name) {
			Ok(Some(module)) => Ok(Some(f(module))),
			Ok(None) => Ok(None),
			Err(error) => Err(error),
		}
	}

	fn find_object(&self, name: &CStr) -> Result<Option<Object<'_>>, Error> {
		match Module::find(name) {
			Ok(Some(module)) => Ok(Some(Object::new(module))),
			Ok(None) => Ok(None),
			Err(error) => Err(error),
		}
	}
}

use ::core::{
	ffi::CStr,
	fmt,
	mem::ManuallyDrop,
	num::NonZero,
	ops::{
		ControlFlow,
		BitAnd, BitOr,
	},
	slice::{
		from_raw_parts, from_raw_parts_mut,
	},
};
use ::libc::{
	dl_iterate_phdr,
	dl_phdr_info,
	c_int, c_void, size_t,
	PF_X, PF_W, PF_R,
};

use crate::map::*;

mod library;
pub use library::*;

macro_rules! for_each_object_callback {
	{
		fn $name:ident<$life:lifetime>:
		Object = $object:ty;
		new_object = $new_object:expr;
	} => {
		unsafe extern "C" fn $name<$life, R, F>(
			info: *mut dl_phdr_info,
			size: size_t,
			data: *mut c_void,
		) -> c_int
		where
			R: ForEachObjectResult,
			F: FnMut($object) -> R,
		{
			unsafe {
				let _ = size;
				let object = $new_object(info);
				let f = &mut *(data as *mut F);
				match f(object).into_raw() {
					Some(i) => i.get(),
					None => 0,
				}
			}
		}
	};
}

pub use ::core::convert::Infallible as Error;

pub(crate) type ModuleName<'a> = &'a CStr;

#[derive(Debug)]
#[repr(transparent)]
pub struct Object<'a>(&'a UnixObject);
impl super::ObjectImpl for Object<'_> {
	fn is_main_program(&self) -> bool {
		self.0.is_main_program()
	}
	fn base_addr(&self) -> usize {
		self.0.base_addr()
	}
	fn segments(&self) -> Segments<'_> {
		Segments {
			headers: self.0.headers().iter(),
		}
	}
	fn symbols(&self) -> Symbols {
		match Symbols::open(self.0.name()) {
			Ok(symbols) => symbols,
			Err(error) => {
				panic!("`dlopen` on a loaded object failed: {error}")
			}
		}
	}
	fn symbol(&self, symbols: &Symbols, name: &CStr) -> *mut () {
		symbols.symbol(name) as _
	}
	fn library(&self, symbols: Symbols) -> Library {
		Library::new(self.0, symbols)
	}
}

#[repr(transparent)]
pub struct Segments<'a> {
	headers: ::core::slice::Iter<'a, ElfSegmentHeader>,
}

impl<'a> Iterator for Segments<'a> {
	type Item = Segment<'a>;
	fn next(&mut self) -> Option<Self::Item> {
		self.headers.next()
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
	fn find_map<R, F: FnMut(ModuleName<'_>, Object<'_>) -> Option<R>>(&self, f: F) -> Result<Option<R>, Error> {
		Ok(Objects::find_map(self, f))
	}
	fn fill_map<'a, M: ?Sized + ObjectMap<'a>>(&self, map: &mut M) -> Result<(), Error> {
		Objects::fill_map(self, map);
		Ok(())
	}
	fn map_by_name<R, F: FnOnce(Object<'_>) -> R>(&self, name: &CStr, f: F) -> Result<Option<R>, Error> {
		Ok(Objects::map_by_name(self, name, f))
	}
	fn for_each<F: FnMut(ModuleName<'_>, Object<'_>) -> bool>(&self, mut f: F) -> Result<(), self::Error> {
		Objects::for_each_object(self, &mut move |object| f(object.name(), Object(object)));
		Ok(())
	}
}

impl Objects {
	pub const fn new() -> Self {
		Self
	}

	pub fn map_by_name<R, F: FnOnce(Object<'_>) -> R>(&self, name: &CStr, f: F) -> Option<R> {
		let name_bytes = name.to_bytes();
		let mut result = None;
		let mut result_mut = &mut result;
		let mut f_once = ManuallyDrop::new(f);
		let _ = self.for_each_object(&mut move |object| {
			if crate::util::check_lib_name(object.name().to_bytes(), name_bytes) {
				// SAFETY: We end the iteration after this by returning `ControlFlow::Break`.
				unsafe {
					let f = ManuallyDrop::take(&mut f_once);
					*result_mut = Some(f(Object(object)));
				}
				ControlFlow::Break(())
			} else {
				ControlFlow::Continue(())
			}
		});
		result
	}

	pub fn find_map<R, F: FnMut(ModuleName<'_>, Object<'_>) -> Option<R>>(&self, mut f: F) -> Option<R> {
		let mut result = None;
		let mut result_mut = &mut result;
		let _ = self.for_each_object(&mut move |object| {
			if let Some(t) = f(object.name(), Object(object)) {
				*result_mut = Some(t);
				ControlFlow::Break(())
			} else {
				ControlFlow::Continue(())
			}
		});
		result
	}

	pub fn fill_map<'a, M: ?Sized + ObjectMap<'a>>(&self, map: &mut M) {
		let _ = self.for_each_object(&mut move |object| {
			if map.is_full() {
				return ControlFlow::Break(())
			}
			let name = object.name();
			for mut entry in map.entries_mut() {
				if entry.name_matches(name) {
					entry.write(crate::Object(Object(object)));
				}
			}
			ControlFlow::Continue(())
		});
	}

	pub fn for_each_object<R, F>(&self, f: &mut F) -> R
	where
		R: ForEachObjectResult,
		F: FnMut(&UnixObject) -> R,
	{
		for_each_object_callback! {
			fn callback<'a>:
			Object = &'a UnixObject;
			new_object = UnixObject::from_ptr;
		}

		unsafe {
			R::from_raw(
				RawFeorInner::new(dl_iterate_phdr(Some(callback::<R, F>), f as *mut F as _))
			)
		}
	}
}

super::transparent_wrapper! {
	pub struct UnixObject for dl_phdr_info;
}
impl UnixObject {
	pub fn is_main_program(&self) -> bool {
		self.name().is_empty()
	}

	pub const fn base_addr(&self) -> usize {
		self.0.dlpi_addr as _
	}

	pub const fn name(&self) -> &CStr {
		unsafe { CStr::from_ptr(self.0.dlpi_name) }
	}

	pub const fn n_headers(&self) -> usize {
		self.0.dlpi_phnum as _
	}

	pub const fn headers(&self) -> &[ElfSegmentHeader] {
		unsafe { from_raw_parts(self.0.dlpi_phdr as *const ElfSegmentHeader, self.n_headers()) }
	}

	pub const fn headers_mut(&mut self) -> &mut [ElfSegmentHeader] {
		unsafe { from_raw_parts_mut(self.0.dlpi_phdr as *mut ElfSegmentHeader, self.n_headers()) }
	}
}

impl fmt::Debug for UnixObject {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("UnixObject")
			.field("base_addr", &format_args!("{:x}", self.base_addr()))
			.field("name", &self.name())
			.finish()
	}
}

super::transparent_wrapper! {
	#[derive(Clone, Copy)]
	pub struct ElfSegmentHeader for ElfPhdr;
}
impl ElfSegmentHeader {
	pub const fn virtual_addr(&self) -> usize {
		self.0.p_vaddr as _
	}

	pub const fn flags(&self) -> SegmentFlags {
		SegmentFlags(self.0.p_flags)
	}

	pub const fn size(&self) -> usize {
		self.0.p_memsz as _
	}
}

pub(crate) type Segment<'a> = &'a ElfSegmentHeader;
impl super::SegmentImpl for Segment<'_> {
	fn flags(&self) -> self::SegmentFlags {
		ElfSegmentHeader::flags(self)
	}
	fn virtual_addr(&self) -> usize {
		ElfSegmentHeader::virtual_addr(self)
	}
	fn size(&self) -> usize {
		ElfSegmentHeader::size(self)
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct SegmentFlags(pub ElfWord);
impl super::SegmentFlagsImpl for SegmentFlags {
	fn has_x(&self) -> bool {
		self.contains(&Self::EXECUTABLE)
	}
	fn has_r(&self) -> bool {
		self.contains(&Self::READABLE)
	}
	fn has_w(&self) -> bool {
		self.contains(&Self::WRITABLE)
	}
	fn is_rx(&self) -> bool {
		self.is(&Self::READABLE.union(Self::EXECUTABLE))
	}
}

impl SegmentFlags {
	pub const EXECUTABLE: Self = Self(PF_X);
	pub const WRITABLE: Self = Self(PF_W);
	pub const READABLE: Self = Self(PF_R);

	pub const fn union(self, other: Self) -> Self {
		Self(self.0 | other.0)
	}

	pub const fn is(&self, other: &Self) -> bool {
		self.0 == other.0
	}

	pub const fn contains(&self, other: &Self) -> bool {
		(self.0 & other.0) != 0
	}

	pub const fn is_executable(&self) -> bool {
		self.contains(&Self::EXECUTABLE)
	}

	pub const fn is_writable(&self) -> bool {
		self.contains(&Self::WRITABLE)
	}

	pub const fn is_readable(&self) -> bool {
		self.contains(&Self::READABLE)
	}
}

impl BitAnd for SegmentFlags {
	type Output = bool;
	fn bitand(self, rhs: Self) -> Self::Output {
		self.contains(&rhs)
	}
}

impl BitOr for SegmentFlags {
	type Output = Self;
	fn bitor(self, rhs: Self) -> Self::Output {
		self.union(rhs)
	}
}

type RawFeorInner = NonZero<c_int>;
pub type RawFeor = Option<RawFeorInner>;

pub trait ForEachObjectResult {
	/// # Safety
	/// `raw` must be a value previously returned by [`into_raw`](ForEachObjectResult::into_raw).
	unsafe fn from_raw(raw: RawFeor) -> Self;
	fn into_raw(self) -> RawFeor;
}

impl ForEachObjectResult for () {
	unsafe fn from_raw(raw: RawFeor) -> Self {
		let _ = raw;
	}
	fn into_raw(self) -> RawFeor {
		None
	}
}

impl ForEachObjectResult for RawFeor {
	unsafe fn from_raw(raw: RawFeor) -> Self {
		raw
	}
	fn into_raw(self) -> RawFeor {
		self
	}
}

impl ForEachObjectResult for bool {
	unsafe fn from_raw(raw: RawFeor) -> Self {
		raw.is_some()
	}
	fn into_raw(self) -> RawFeor {
		if self {
			// SAFETY: 1 != 0
			unsafe { Some(RawFeorInner::new_unchecked(1)) }
		} else {
			None
		}
	}
}

impl ForEachObjectResult for ControlFlow<()> {
	unsafe fn from_raw(raw: RawFeor) -> Self {
		unsafe {
			if bool::from_raw(raw) {
				ControlFlow::Break(())
			} else {
				ControlFlow::Continue(())
			}
		}
	}
	fn into_raw(self) -> RawFeor {
		self.is_break().into_raw()
	}
}

macro_rules! bit_dependent_type {
	{
		$vis:vis type $name:ident:
		32 => $b32:ty;
		64 => $b64:ty;
	} => {
		#[cfg(target_pointer_width = "32")]
		$vis type $name = $b32;
		#[cfg(target_pointer_width = "64")]
		$vis type $name = $b64;
	};
}

bit_dependent_type! {
	type ElfPhdr:
	32 => ::libc::Elf32_Phdr;
	64 => ::libc::Elf64_Phdr;
}

bit_dependent_type! {
	type ElfWord:
	32 => ::libc::Elf32_Word;
	64 => ::libc::Elf64_Word;
}

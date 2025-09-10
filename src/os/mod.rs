use ::core::ffi::CStr;

#[cfg(unix)]
pub mod unix;
#[cfg(windows)]
pub mod windows;

#[cfg(unix)]
pub(crate) use unix as imp;

#[cfg(windows)]
pub(crate) use windows as imp;

#[cfg(not(any(unix, windows)))]
compile_error!("only `unix` and `windows` targets are supported");

pub(crate) trait SegmentFlagsImpl: Sized + Copy {
	fn has_x(&self) -> bool;
	fn has_r(&self) -> bool;
	fn has_w(&self) -> bool;
	fn is_rx(&self) -> bool;
}

pub(crate) trait SegmentImpl {
	fn flags(&self) -> imp::SegmentFlags;
	fn virtual_addr(&self) -> usize;
	fn size(&self) -> usize;
}

pub(crate) trait LibraryImpl {
	fn base_addr(&self) -> usize;
	fn symbol(&self, name: &CStr) -> *mut ();
}

pub(crate) trait ObjectImpl {
	fn base_addr(&self) -> usize;
	fn segments(&self) -> imp::Segments<'_>;
	fn symbols(&self) -> Option<imp::Symbols>;
	fn symbol(&self, symbols: &imp::Symbols, name: &CStr) -> *mut ();
	fn library(&self, symbols: imp::Symbols) -> imp::Library;
}

pub(crate) trait ObjectsImpl
where
	for<'a> imp::Object<'a>: ObjectImpl,
	for<'a> imp::ModuleName<'a>: ModuleNameImpl,
	for<'a> imp::Segments<'a>: Iterator<Item = imp::Segment<'a>>,
	for<'a> imp::Segment<'a>: SegmentImpl,
	imp::Library: LibraryImpl,
	imp::SegmentFlags: SegmentFlagsImpl,
{
	fn init() -> Self;
	fn find_map<R, F: FnMut(imp::ModuleName<'_>, imp::Object<'_>) -> Option<R>>(
		&self, f: F,
	) -> Result<Option<R>, imp::Error>;
	fn fill_map<'a, M: ?Sized + crate::map::ObjectMap<'a>>(&self, map: &mut M) -> Result<(), imp::Error>;
	fn map_by_name<R, F: FnOnce(imp::Object<'_>) -> R>(&self, name: &CStr, f: F) -> Result<Option<R>, imp::Error>;
	fn for_each<F: FnMut(imp::ModuleName<'_>, imp::Object<'_>) -> bool>(&self, f: F) -> Result<(), imp::Error>;
}

pub(crate) trait ModuleNameImpl {
	fn as_c_str(&self) -> &CStr;
}
impl<T: ?Sized + AsRef<CStr>> super::ModuleNameImpl for T {
	fn as_c_str(&self) -> &CStr {
		self.as_ref()
	}
}

#[allow(unused_macros)]
macro_rules! transparent_wrapper {
	{
		$(#[$attr:meta])*
		$vis:vis struct $name:ident for $target:ty;
	} => {
		#[repr(transparent)]
		$(#[$attr])*
		$vis struct $name($target);
		impl $name {
			/// Returns an immutable reference to the inner structure.
			pub const fn as_inner(&self) -> &$target {
				&self.0
			}

			/// Returns an immutable reference to this structure from a pointer.
			/// 
			/// # Safety
			/// `ptr` must point to a valid, readable inner structure.
			pub const unsafe fn from_ptr<'a>(ptr: *mut $target) -> &'a Self {
				unsafe { &*(ptr as *const Self) }
			}
		}
	};
}
#[allow(unused_imports)]
pub(crate) use transparent_wrapper;

#[allow(unused_macros)]
macro_rules! lifetime_wrapper {
	{
		$(#[$attr:meta])*
		$vis:vis struct $name:ident for $target:ty;
	} => {
		#[repr(transparent)]
		$(#[$attr])*
		$vis struct $name<'a> {
			inner: $target,
			_life: ::core::marker::PhantomData<&'a $target>,
		}

		impl $name<'_> {
			pub const fn new(inner: $target) -> Self {
				Self {
					inner,
					_life: ::core::marker::PhantomData,
				}
			}
		}
	};
}
#[allow(unused_imports)]
pub(crate) use lifetime_wrapper;

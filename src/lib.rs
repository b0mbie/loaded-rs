#![allow(dead_code)]

use ::core::ffi::CStr;

pub mod util;

pub mod map;
use map::*;
pub mod os;
use os::*;

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
#[repr(transparent)]
pub struct Error(imp::Error);

#[derive(Debug)]
#[repr(transparent)]
pub struct Object<'a>(imp::Object<'a>);
impl Object<'_> {
	pub fn is_main_program(&self) -> bool {
		ObjectImpl::is_main_program(&self.0)
	}

	pub fn base_addr(&self) -> usize {
		ObjectImpl::base_addr(&self.0)
	}

	pub fn segments(&self) -> Segments<'_> {
		Segments(ObjectImpl::segments(&self.0))
	}

	pub fn symbols(&self) -> Symbols {
		Symbols(ObjectImpl::symbols(&self.0))
	}

	pub fn symbol(&self, symbols: &Symbols, name: &CStr) -> *mut () {
		ObjectImpl::symbol(&self.0, &symbols.0, name)
	}

	pub fn library(&self, symbols: Symbols) -> Library {
		Library(ObjectImpl::library(&self.0, symbols.0))
	}
}

#[derive(Debug)]
#[repr(transparent)]
pub struct Symbols(imp::Symbols);

#[derive(Debug)]
#[repr(transparent)]
pub struct Library(imp::Library);
impl Library {
	pub fn base_addr(&self) -> usize {
		LibraryImpl::base_addr(&self.0)
	}

	pub fn symbol(&self, name: &CStr) -> *mut () {
		LibraryImpl::symbol(&self.0, name)
	}
}

#[repr(transparent)]
pub struct Segments<'a>(imp::Segments<'a>);
impl<'a> Iterator for Segments<'a> {
	type Item = Segment<'a>;
	fn next(&mut self) -> Option<Self::Item> {
		self.0.next().map(Segment)
	}
}

#[repr(transparent)]
pub struct Segment<'a>(imp::Segment<'a>);
impl Segment<'_> {
	pub fn flags(&self) -> SegmentFlags {
		SegmentFlags(self.0.flags())
	}

	pub fn virtual_addr(&self) -> usize {
		SegmentImpl::virtual_addr(&self.0)
	}

	pub fn size(&self) -> usize {
		SegmentImpl::size(&self.0)
	}
}

#[repr(transparent)]
pub struct SegmentFlags(imp::SegmentFlags);
impl SegmentFlags {
	pub fn has_x(&self) -> bool {
		SegmentFlagsImpl::has_x(&self.0)
	}

	pub fn has_r(&self) -> bool {
		SegmentFlagsImpl::has_r(&self.0)
	}

	pub fn has_w(&self) -> bool {
		SegmentFlagsImpl::has_w(&self.0)
	}

	pub fn is_rx(&self) -> bool {
		SegmentFlagsImpl::is_rx(&self.0)
	}
}

#[derive(Default)]
#[repr(transparent)]
pub struct Objects(imp::Objects);
impl Objects {
	/// Returns a structure which can be used to query loaded objects.
	pub fn new() -> Self {
		Self(ObjectsImpl::init())
	}

	/// Tries to find a loaded object by `name` and applies `f` to it.
	pub fn map_by_name<R, F>(&self, name: &CStr, f: F) -> Result<Option<R>, Error>
	where
		F: FnOnce(Object<'_>) -> R,
	{
		match ObjectsImpl::map_by_name(&self.0, name, move |object| f(Object(object))) {
			Ok(result) => Ok(result),
			Err(inner) => Err(Error(inner)),
		}
	}

	/// Tries to fill `map` with the loaded objects that it is requesting.
	/// 
	/// See the documentation for [`ObjectMap`] for more information.
	pub fn fill_map<'a, M>(&self, map: &mut M) -> Result<(), Error>
	where
		M: ?Sized + ObjectMap<'a>,
	{
		match ObjectsImpl::fill_map(&self.0, map) {
			Ok(result) => Ok(result),
			Err(inner) => Err(Error(inner)),
		}
	}

	/// Iterates over all named loaded objects, applying `f` to them,
	/// and returning the first result that is `Some`.
	pub fn find_map<R, F>(&self, mut f: F) -> Result<Option<R>, Error>
	where
		F: FnMut(&CStr, Object<'_>) -> Option<R>,
	{
		match ObjectsImpl::find_map(&self.0, move |name, object| f(name.as_c_str(), Object(object))) {
			Ok(result) => Ok(result),
			Err(inner) => Err(Error(inner)),
		}
	}

	/// Iterates over all named loaded objects, calling `f` with them.
	/// 
	/// Unlike [`find_map`](Self::find_map),
	/// this method cannot map objects to other kinds of values,
	/// and is specifically designed for inspection.
	pub fn for_each<R, F>(&self, mut f: F) -> Result<(), Error>
	where
		R: ForEachResult,
		F: FnMut(&CStr, Object<'_>) -> R,
	{
		match ObjectsImpl::for_each(&self.0, move |name, object| f(name.as_c_str(), Object(object)).into_is_break()) {
			Ok(result) => Ok(result),
			Err(inner) => Err(Error(inner)),
		}
	}
}

/// Trait for values that can be returned in callbacks in [`Objects::for_each`].
pub trait ForEachResult {
	fn into_is_break(self) -> bool;
}
impl ForEachResult for bool {
	fn into_is_break(self) -> bool {
		self
	}
}
impl ForEachResult for () {
	fn into_is_break(self) -> bool {
		false
	}
}
impl ForEachResult for ::core::ops::ControlFlow<(), ()> {
	fn into_is_break(self) -> bool {
		self.is_break()
	}
}

#[cfg(test)]
mod tests {
	use crate::*;

	#[test]
	fn has_main_program() {
		let objects = Objects::new();
		let mut has_main_program = false;
		objects.for_each(|_, object| {
			let is_main_program = object.is_main_program();
			has_main_program = is_main_program;
			is_main_program
		}).unwrap();
		assert!(has_main_program);
	}

	#[test]
	fn cant_find_invalid() {
		let objects = Objects::new();
		assert_eq!(objects.map_by_name(c"\n", move |_| ()).unwrap(), None);
	}
}

use ::core::{
	ffi::CStr,
	iter::{
		Once, once,
	},
	slice::{
		Iter, IterMut,
	},
};

use crate::{
	util::to_nice_name,
	Object,
};

/// Returns a closure that takes possible object names and matches them with the `object_name`.
/// 
/// More specifically, the function will 
/// do a [`PartialEq`] comparison of the bytes of `object_name`
/// with [`to_nice_name`] and then `name`.
pub fn default_name_matcher(object_name: &CStr) -> impl Fn(&CStr) -> bool {
	let original_name = object_name.to_bytes();
	let nice_name = to_nice_name(original_name);
	move |name| {
		let name = name.to_bytes();
		name == nice_name || name == original_name
	}
}

pub trait ObjectMap<'object> {
	type Entry<'iter>: ObjectMapEntry where Self: 'iter;
	type Entries<'iter>: Iterator<Item = Self::Entry<'iter>> where Self: 'iter;
	fn entries(&self) -> Self::Entries<'_>;

	type EntryMut<'iter>: ObjectMapEntryMut where Self: 'iter;
	type EntriesMut<'iter>: Iterator<Item = Self::EntryMut<'iter>> where Self: 'iter;
	fn entries_mut(&mut self) -> Self::EntriesMut<'_>;

	fn is_full(&self) -> bool {
		self.entries().all(move |entry| entry.is_written())
	}
}

pub trait ObjectMapEntry {
	/// Iterator over all potential names to be stored in this entry.
	/// 
	/// See documentation for [`names`](ObjectMapEntry::names) for more information.
	type Names<'a>: Iterator<Item = &'a CStr> where Self: 'a;

	/// Returns all potential names to be stored in this entry.
	/// 
	/// # Platform usage
	/// This is used on Windows to query for specific modules (with `GetModuleHandle`)
	/// without iterating over all of them
	/// and potentially allocating resources unnecessarily.
	/// More specifically, each of the names yielded by the iterator will be queried for,
	/// and the first one that exists will be written to the entry.
	/// If there is only one possible name, then [`Once`] can be used.
	fn names(&self) -> Self::Names<'_>;

	/// Returns `true` if the entry has already been written to.
	fn is_written(&self) -> bool;

	/// Returns `true` if `name` is *partially* equivalent the name of the loaded library for the entry.
	/// 
	/// # Platform usage
	/// This is used on Unix while iterating over all modules (with `dl_iterate_phdr`)
	/// to compare a loaded object's name to the one that the entry could store.
	/// 
	/// # Default implementation
	/// The default implementation tries to match `name`
	/// with one of the entry's [`names`](ObjectMapEntry::names)
	/// using [`default_name_matcher`].
	/// However, the behavior of this method can be arbitrary.
	fn name_matches(&self, name: &CStr) -> bool {
		self.names().any(default_name_matcher(name))
	}
}
impl<T: ?Sized + ObjectMapEntry> ObjectMapEntry for &T {
	type Names<'a> = T::Names<'a> where Self: 'a;
	fn names(&self) -> Self::Names<'_> {
		T::names(self)
	}
	fn is_written(&self) -> bool {
		T::is_written(self)
	}

	fn name_matches(&self, name: &CStr) -> bool {
		T::name_matches(self, name)
	}
}
impl<T: ?Sized + ObjectMapEntry> ObjectMapEntry for &mut T {
	type Names<'a> = T::Names<'a> where Self: 'a;
	fn names(&self) -> Self::Names<'_> {
		T::names(self)
	}
	fn is_written(&self) -> bool {
		T::is_written(self)
	}

	fn name_matches(&self, name: &CStr) -> bool {
		T::name_matches(self, name)
	}
}

pub trait ObjectMapEntryMut: ObjectMapEntry {
	fn write(&mut self, object: Object<'_>);
}
impl<T: ?Sized + ObjectMapEntryMut> ObjectMapEntryMut for &mut T {
	fn write(&mut self, object: Object<'_>) {
		T::write(self, object)
	}
}

impl<T> ObjectMap<'_> for T
where
	T: ObjectMapEntryMut,
{
	type Entry<'iter> = &'iter T where Self: 'iter;
	type Entries<'iter> = Once<&'iter T> where Self: 'iter;
	fn entries(&self) -> Self::Entries<'_> {
		once(self)
	}

	type EntryMut<'iter> = &'iter mut T where Self: 'iter;
	type EntriesMut<'iter> = Once<&'iter mut T> where Self: 'iter;
	fn entries_mut(&mut self) -> Self::EntriesMut<'_> {
		once(self)
	}
}

impl<T> ObjectMap<'_> for [T]
where
	T: ObjectMapEntryMut,
{
	type Entry<'iter> = &'iter T where Self: 'iter;
	type Entries<'iter> = Iter<'iter, T> where Self: 'iter;
	fn entries(&self) -> Self::Entries<'_> {
		self.iter()
	}

	type EntryMut<'iter> = &'iter mut T where Self: 'iter;
	type EntriesMut<'iter> = IterMut<'iter, T> where Self: 'iter;
	fn entries_mut(&mut self) -> Self::EntriesMut<'_> {
		self.iter_mut()
	}
}
impl<K, V> ObjectMapEntry for (K, Option<V>)
where
	K: AsRef<CStr> + PartialEq<CStr>,
	V: for<'a> From<Object<'a>>,
{
	type Names<'a> = Once<&'a CStr> where Self: 'a;
	fn names(&self) -> Self::Names<'_> {
		once(self.0.as_ref())
	}
	fn is_written(&self) -> bool {
		self.1.is_some()
	}

	fn name_matches(&self, name: &CStr) -> bool {
		self.0 == *name
	}
}
impl<K, V> ObjectMapEntryMut for (K, Option<V>)
where
	K: AsRef<CStr> + PartialEq<CStr>,
	V: for<'a> From<Object<'a>>,
{
	fn write(&mut self, object: Object<'_>) {
		self.1 = Some(V::from(object));
	}
}

pub fn check_lib_name(lib: &[u8], target: &[u8]) -> bool {
	lib == target || to_nice_name(lib) == target
}

pub fn to_nice_name(name: &[u8]) -> &[u8] {
	fn slice_after_last(slice: &[u8], element: u8) -> &[u8] {
		if let Some(index) = slice.iter().enumerate().rev().find_map(move |(index, &t)| (t == element).then_some(index)) {
			unsafe { slice.get_unchecked(index + 1..) }
		} else {
			slice
		}
	}

	fn slice_before_first(slice: &[u8], element: u8) -> &[u8] {
		if let Some(index) = slice.iter().position(move |&t| t == element) {
			unsafe { slice.get_unchecked(..index) }
		} else {
			slice
		}
	}

	let nice_name = slice_after_last(name, std::path::MAIN_SEPARATOR as u8);
	(slice_before_first(nice_name, b'.')) as _
}

use std::convert::TryInto;
use std::fmt::Debug;

/// Convert `n` to `u8` using `TryFrom` or panic.
///
/// # Panics
/// Panics if the conversion returns an error.
pub fn cast_u8<T>(n: T) -> u8
where
    T: TryInto<u8>,
    <T as TryInto<u8>>::Error: Debug,
{
    n.try_into().expect("Expected N to fit in u8")
}

/// Convert `n` to `u32` using `TryFrom` or panic.
///
/// # Panics
/// Panics if the conversion returns an error.
pub fn cast_u32<T>(n: T) -> u32
where
    T: TryInto<u32>,
    <T as TryInto<u32>>::Error: Debug,
{
    n.try_into().expect("Expected N to fit in u32")
}

/// Convert `n` to `i32` using `TryFrom` or panic.
///
/// # Panics
/// Panics if the conversion returns an error.
pub fn cast_i32<T>(n: T) -> i32
where
    T: TryInto<i32>,
    <T as TryInto<i32>>::Error: Debug,
{
    n.try_into().expect("Expected N to fit in i32")
}

/// Convert `n` to `usize` using `TryFrom` or panic.
///
/// # Panics
/// Panics if the conversion returns an error.
pub fn cast_usize<T>(n: T) -> usize
where
    T: TryInto<usize>,
    <T as TryInto<usize>>::Error: Debug,
{
    n.try_into().expect("Expected N to fit in usize")
}

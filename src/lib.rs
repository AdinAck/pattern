#![no_std]

use core::mem::MaybeUninit;
use tiny_serde::Deserialize;
#[cfg(feature = "defmt")]
use defmt::Format;

#[cfg_attr(feature = "defmt", derive(Format))]
pub enum PatternError {
    NotFound, // end of iter was reached when looking for value
    FailedDeserialize(usize), // type could not be deserialized from data
}

/// Expects N values of any value immediately.
pub struct AnyStrategy<'a, I, const N: usize>
where
    I: Iterator,
{
    pattern: &'a mut Pattern<I>,
}

/// Scans for N bytes and extracts them if available.
impl<'a, I, const N: usize> AnyStrategy<'a, I, N>
where
    I: Iterator
{
    fn new(pattern: &'a mut Pattern<I>) -> Self {
        Self { pattern }
    }

    /// Extracts the (consumed) values that were expected (or a [PatternError]).
    pub fn extract_and<F>(&mut self, mut closure: F) -> Result<[I::Item; N], PatternError>
    where
        F: FnMut(&[I::Item])
    {
        let mut result = unsafe { MaybeUninit::<[I::Item; N]>::uninit().assume_init() };

        self.pattern.collect(N, |i, candidate| {
            result[i] = candidate;

            Ok(())
        })?;

        let result = result;

        closure(&result);

        Ok(result)
    }

    #[inline]
    pub fn extract(&mut self) -> Result<[I::Item; N], PatternError> {
        self.extract_and(|_| { })
    }
}

/// Scans for types that implement `tiny_serde::Deserialize` and extracts N of them.
pub struct GetStrategy<'a, I, const N: usize>
where
    I: Iterator
{
    pattern: &'a mut Pattern<I>
}

impl<'a, I, const N: usize> GetStrategy<'a, I, N>
where
    I: Iterator<Item = u8>,
{
    fn new(pattern: &'a mut Pattern<I>) -> Self {
        Self { pattern }
    }

    pub fn extract_and<T, const K: usize, F>(&mut self, mut closure: F) -> Result<[T; N], PatternError>
    where
        T: Deserialize<K>,
        F: FnMut(&[I::Item])
    {
        let mut result = unsafe { MaybeUninit::<[T; N]>::uninit().assume_init() };

        for i in 0..N {
            if let Some(value) = T::deserialize(self.pattern.any().extract_and(&mut closure)?) {
                result[i] = value;
            } else {
                return Err(PatternError::FailedDeserialize(self.pattern.count()))
            }
        }

        Ok(result)
    }

    #[inline]
    pub fn extract<T, const K: usize>(&mut self) -> Result<[T; N], PatternError>
    where
        T: Deserialize<K> + Copy,
    {
        self.extract_and(|_| { })
    }
}

/// Facilitates the extraction and validation of desired sequences of items from an iterator.
#[derive(Clone)]
pub struct Pattern<I>
where
    I: Iterator
{
    iter: I,
    count: usize,
}

impl<I> Pattern<I>
where
    I: Iterator,
{
    pub fn new(iter: I) -> Self {
        Self {
            iter,
            count: 0
        }
    }

    /// Calls a provided closure on the next items of the iterator for a given count.
    /// Also increments the expected size and actual size of collected values.
    fn collect<F: FnMut(usize, I::Item) -> Result<(), PatternError>>(&mut self, count: usize, mut callback: F) -> Result<(), PatternError> {
        for i in 0..count {
            if let Some(candidate) = self.iter.next() {
                if let Err(e) = callback(i, candidate) {
                    self.count += i;
                    return Err(e);
                }
            } else {
                self.count += i;
                return Err(PatternError::NotFound);
            }
        }

        self.count += count;

        Ok(())
    }

    /// Dispatches an [AnyStrategy].
    fn any<const N: usize>(&mut self) -> AnyStrategy<I, N> {
        AnyStrategy::new(self)
    }

    /// Dispatches a [GetStrategy].
    pub fn get<'a, const N: usize>(&mut self) -> GetStrategy<I, N>
    where
        I: Iterator<Item = u8>
    {
        GetStrategy::new(self)
    }

    pub fn count(&self) -> usize {
        self.count
    }
}
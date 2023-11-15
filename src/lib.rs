#![no_std]

use core::mem::MaybeUninit;
use tiny_serde::Deserialize;
#[cfg(feature = "defmt")]
use defmt::Format;

#[cfg_attr(feature = "defmt", derive(Format))]
pub enum PatternError {
    NotFound, // end of iter was reached when looking for value
    FailedDeserialize, // type could not be deserialized from data
    IncorrectValue // expected value was not read
}

/// Expects an immediate sequence of given values in order.
pub struct ImmediateValueStrategy<'a, I, const N: usize>
where
    I: Iterator,
{
    pattern: &'a mut Pattern<I>,
    values: [I::Item; N],
}

impl<'a, I, const N: usize> ImmediateValueStrategy<'a, I, N>
where
    I: Iterator,
    I::Item: PartialEq + Copy,
{
    fn new(pattern: &'a mut Pattern<I>, values: [I::Item; N]) -> Self {
        Self { pattern, values }
    }

    /// Converts the [ImmediateValueStrategy] strategy into a [DeferredValueStrategy].
    pub fn deferred(self) -> DeferredValueStrategy<'a, I, N> {
        self.into()
    }

    /// Extracts the (consumed) values that were expected (or a [PatternError]).
    pub fn extract_and<F>(&mut self, mut closure: F) -> Result<[I::Item; N], PatternError>
    where
        F: FnMut(&I::Item)
    {
        let mut result = [unsafe { MaybeUninit::uninit().assume_init() }; N];

        self.pattern.collect(N, |i, candidate| {
            if candidate == self.values[i] {
                result[i] = candidate;

                closure(&candidate);

                Ok(())
            } else {
                Err(PatternError::IncorrectValue)
            }
        })?;

        Ok(result)
    }

    #[inline]
    pub fn extract(&mut self) -> Result<[I::Item; N], PatternError> {
        self.extract_and(|_| { })
    }
}

/// Expects a sequence of given values whenever discovered later.
pub struct DeferredValueStrategy<'a, I, const N: usize>
where
    I: Iterator,
{
    pattern: &'a mut Pattern<I>,
    values: [I::Item; N],
}

impl<'a, I, const N: usize> DeferredValueStrategy<'a, I, N>
where
    I: Iterator,
    I::Item: PartialEq + Copy,
{
    /// Extracts the (consumed) values that were expected (or a [PatternError]).
    pub fn extract_and<F>(&mut self, mut closure: F) -> Result<[I::Item; N], PatternError>
    where
        F: FnMut(&[I::Item])
    {
        let mut result = [unsafe { MaybeUninit::uninit().assume_init() }; N];

        // find first value
        loop {
            if let Some(candidate) = self.pattern.iter.next() {
                if candidate == self.values[0] {
                    result[0] = candidate;
                    break;
                }
            } else {
                return Err(PatternError::NotFound);
            }
        }

        // collect the rest of the values normally
        self.pattern.collect(N - 1, |i, candidate| {
            if candidate == self.values[i + 1] {
                result[i + 1] = candidate;

                Ok(())
            } else {
                Err(PatternError::IncorrectValue)
            }
        })?;

        closure(&result);

        Ok(result)
    }

    #[inline]
    pub fn extract(&mut self) -> Result<[I::Item; N], PatternError> {
        self.extract_and(|_| { })
    }
}

impl<'a, I, const N: usize> From<ImmediateValueStrategy<'a, I, N>>
    for DeferredValueStrategy<'a, I, N>
where
    I: Iterator,
{
    fn from(value: ImmediateValueStrategy<'a, I, N>) -> Self {
        DeferredValueStrategy {
            pattern: value.pattern,
            values: value.values,
        }
    }
}

/// Expects N values of any value immediately.
pub struct AnyStrategy<'a, I, const N: usize>
where
    I: Iterator,
{
    pattern: &'a mut Pattern<I>,
}

impl<'a, I, const N: usize> AnyStrategy<'a, I, N>
where
    I: Iterator,
    I::Item: Copy
{
    fn new(pattern: &'a mut Pattern<I>) -> Self {
        Self { pattern }
    }

    /// Extracts the (consumed) values that were expected (or a [PatternError]).
    pub fn extract_and<F>(&mut self, mut closure: F) -> Result<[I::Item; N], PatternError>
    where
        F: FnMut(&[I::Item])
    {
        let mut result = [unsafe { MaybeUninit::uninit().assume_init() }; N];

        self.pattern.collect(N, |i, candidate| {
            result[i] = candidate;

            Ok(())
        })?;

        closure(&result);

        Ok(result)
    }

    #[inline]
    pub fn extract(&mut self) -> Result<[I::Item; N], PatternError> {
        self.extract_and(|_| { })
    }
}

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
        T: Deserialize<K> + Copy,
        F: FnMut(&[I::Item])
    {
        let mut result = [unsafe { MaybeUninit::uninit().assume_init() }; N];

        for i in 0..N {
            if let Some(value) = T::deserialize(self.pattern.any().extract_and(&mut closure)?) {
                result[i] = value;
            } else {
                return Err(PatternError::FailedDeserialize)
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
    I: Iterator,
{
    iter: I
}

impl<I> Pattern<I>
where
    I: Iterator,
{
    pub fn new(iter: I) -> Self {
        Self {
            iter
        }
    }

    /// Calls a provided closure on the next items of the iterator for a given count.
    /// Also increments the expected size and actual size of collected values.
    fn collect<F: FnMut(usize, I::Item) -> Result<(), PatternError>>(&mut self, count: usize, mut callback: F) -> Result<(), PatternError> {
        for i in 0..count {
            if let Some(candidate) = self.iter.next() {
                if let Err(e) = callback(i, candidate) {
                    return Err(e);
                }
            } else {
                return Err(PatternError::NotFound);
            }
        }

        Ok(())
    }

    /// Dispatches an [ImmediateValueStrategy].
    pub fn values<const N: usize>(&mut self, values: [I::Item; N]) -> ImmediateValueStrategy<I, N>
    where
        I::Item: PartialEq + Copy
    {
        ImmediateValueStrategy::new(self, values)
    }

    /// Dispatches an [AnyStrategy].
    pub fn any<const N: usize>(&mut self) -> AnyStrategy<I, N>
    where
        I::Item: Copy
    {
        AnyStrategy::new(self)
    }

    pub fn get<'a, const N: usize>(&mut self) -> GetStrategy<I, N>
    where
        I: Iterator<Item = u8>
    {
        GetStrategy::new(self)
    }
}
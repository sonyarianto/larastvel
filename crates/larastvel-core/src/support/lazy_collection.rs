//! Lazy collection — an iterator wrapper with Laravel-style chaining.

use std::iter::Iterator;

/// A lazy, chainable iterator — Laravel's `LazyCollection`.
///
/// Unlike a materialized [`Vec`], values are produced on demand.
///
/// ```rust,ignore
/// let lazy = LazyCollection::new(0..1_000_000)
///     .filter(|n| n % 2 == 0)
///     .map(|n| n * 10)
///     .take(3);
///
/// assert_eq!(lazy.collect_vec(), vec![0, 20, 40]);
/// ```
#[derive(Debug, Clone)]
pub struct LazyCollection<I> {
    inner: I,
}

impl<I: Iterator> LazyCollection<I> {
    pub fn new(inner: I) -> Self {
        Self { inner }
    }

    /// Build from anything that can be turned into an iterator — Laravel's
    /// `LazyCollection::from()`.
    pub fn from<U: IntoIterator<IntoIter = I>>(iter: U) -> Self {
        Self {
            inner: iter.into_iter(),
        }
    }

    pub fn filter<P>(self, predicate: P) -> LazyCollection<std::iter::Filter<I, P>>
    where
        P: FnMut(&I::Item) -> bool,
    {
        LazyCollection {
            inner: self.inner.filter(predicate),
        }
    }

    pub fn map<B, F>(self, f: F) -> LazyCollection<std::iter::Map<I, F>>
    where
        F: FnMut(I::Item) -> B,
    {
        LazyCollection {
            inner: self.inner.map(f),
        }
    }

    pub fn take(self, n: usize) -> LazyCollection<std::iter::Take<I>> {
        LazyCollection {
            inner: self.inner.take(n),
        }
    }

    pub fn skip(self, n: usize) -> LazyCollection<std::iter::Skip<I>> {
        LazyCollection {
            inner: self.inner.skip(n),
        }
    }

    pub fn chain<U>(self, other: U) -> LazyCollection<std::iter::Chain<I, U::IntoIter>>
    where
        U: IntoIterator<Item = I::Item>,
    {
        LazyCollection {
            inner: self.inner.chain(other),
        }
    }

    pub fn collect_vec(self) -> Vec<I::Item> {
        self.inner.collect()
    }

    pub fn count(self) -> usize {
        self.inner.count()
    }

    /// Reduce the collection into a single value — Laravel's `reduce()`.
    pub fn reduce<F>(self, f: F) -> Option<I::Item>
    where
        F: FnMut(I::Item, I::Item) -> I::Item,
    {
        self.inner.reduce(f)
    }
}

impl<I: Iterator> Iterator for LazyCollection<I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lazy_chaining() {
        let lazy = LazyCollection::new(0..1_000_000)
            .filter(|n| n % 2 == 0)
            .map(|n| n * 10)
            .take(3);
        assert_eq!(lazy.collect_vec(), vec![0, 20, 40]);
    }

    #[test]
    fn test_lazy_is_lazy() {
        let mut calls = 0;
        let lazy = LazyCollection::new(0..100)
            .map(|n| {
                calls += 1;
                n
            })
            .take(2);
        let _ = lazy.collect_vec();
        assert_eq!(calls, 2);
    }

    #[test]
    fn test_lazy_skip_chain() {
        let lazy = LazyCollection::from(vec![1, 2, 3, 4])
            .skip(1)
            .chain(vec![9, 10]);
        assert_eq!(lazy.collect_vec(), vec![2, 3, 4, 9, 10]);
    }

    #[test]
    fn test_lazy_reduce() {
        let sum = LazyCollection::from(vec![1, 2, 3, 4]).reduce(|a, b| a + b);
        assert_eq!(sum, Some(10));
    }
}

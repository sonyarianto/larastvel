use std::cmp::Ordering;
use std::hash::Hash;

#[derive(Debug, Clone)]
pub struct Collection<T> {
    items: Vec<T>,
}

impl<T> Collection<T> {
    pub fn new(items: Vec<T>) -> Self {
        Self { items }
    }

    pub fn collect<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self {
            items: iter.into_iter().collect(),
        }
    }

    pub fn items(&self) -> &Vec<T> {
        &self.items
    }

    pub fn into_items(self) -> Vec<T> {
        self.items
    }

    pub fn count(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn first(&self) -> Option<&T> {
        self.items.first()
    }

    pub fn last(&self) -> Option<&T> {
        self.items.last()
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.items.get(index)
    }

    pub fn map<U, F: FnMut(&T) -> U>(&self, f: F) -> Collection<U> {
        Collection::new(self.items.iter().map(f).collect())
    }

    pub fn filter<F: FnMut(&T) -> bool>(&self, mut f: F) -> Collection<&T> {
        Collection::new(self.items.iter().filter(|i| f(i)).collect())
    }

    pub fn reduce<F: FnMut(T, T) -> T>(self, f: F) -> Option<T> {
        self.items.into_iter().reduce(f)
    }

    pub fn fold<B, F: FnMut(B, T) -> B>(self, init: B, f: F) -> B {
        self.items.into_iter().fold(init, f)
    }

    pub fn sort_by<F: FnMut(&T, &T) -> Ordering>(mut self, f: F) -> Self {
        self.items.sort_by(f);
        self
    }

    pub fn reverse(mut self) -> Self {
        self.items.reverse();
        self
    }

    pub fn take(&self, n: usize) -> Collection<&T> {
        Collection::new(self.items.iter().take(n).collect())
    }

    pub fn skip(&self, n: usize) -> Collection<&T> {
        Collection::new(self.items.iter().skip(n).collect())
    }

    pub fn unique<F: FnMut(&T) -> U, U: Hash + Eq>(&self, mut f: F) -> Collection<&T> {
        let mut seen = std::collections::HashSet::new();
        Collection::new(
            self.items
                .iter()
                .filter(move |item| seen.insert(f(item)))
                .collect(),
        )
    }

    pub fn chunk(&self, size: usize) -> Collection<Vec<&T>> {
        Collection::new(
            self.items
                .chunks(size)
                .map(|c| c.iter().collect())
                .collect(),
        )
    }

    pub fn each<F: FnMut(&T)>(&self, f: F) {
        self.items.iter().for_each(f);
    }

    pub fn to_json(&self) -> serde_json::Value
    where
        T: serde::Serialize,
    {
        serde_json::to_value(&self.items).unwrap_or(serde_json::Value::Null)
    }
}

impl<T: Clone> Collection<T> {
    pub fn sort(mut self) -> Self
    where
        T: Ord,
    {
        self.items.sort();
        self
    }

    pub fn pluck<U: Clone, F: Fn(&T) -> U>(&self, f: F) -> Collection<U> {
        Collection::new(self.items.iter().map(f).collect())
    }
}

impl<T: Clone> Collection<&T> {
    pub fn cloned(&self) -> Collection<T> {
        Collection::new(self.items.iter().map(|&i| i.clone()).collect())
    }
}

impl<T> From<Vec<T>> for Collection<T> {
    fn from(items: Vec<T>) -> Self {
        Self { items }
    }
}

impl<T> From<Collection<T>> for Vec<T> {
    fn from(val: Collection<T>) -> Self {
        val.items
    }
}

impl<T> IntoIterator for Collection<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a Collection<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

pub fn collect<T, I: IntoIterator<Item = T>>(iter: I) -> Collection<T> {
    Collection::new(iter.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_and_count() {
        let c = Collection::new(vec![1, 2, 3]);
        assert_eq!(c.count(), 3);
        assert!(!c.is_empty());
    }

    #[test]
    fn test_empty() {
        let c: Collection<i32> = Collection::new(vec![]);
        assert!(c.is_empty());
        assert_eq!(c.count(), 0);
    }

    #[test]
    fn test_first_and_last() {
        let c = Collection::new(vec![1, 2, 3]);
        assert_eq!(c.first(), Some(&1));
        assert_eq!(c.last(), Some(&3));
    }

    #[test]
    fn test_get() {
        let c = Collection::new(vec![10, 20, 30]);
        assert_eq!(c.get(0), Some(&10));
        assert_eq!(c.get(2), Some(&30));
        assert_eq!(c.get(5), None);
    }

    #[test]
    fn test_map() {
        let c = Collection::new(vec![1, 2, 3]);
        let mapped = c.map(|x| x * 2);
        assert_eq!(mapped.into_items(), vec![2, 4, 6]);
    }

    #[test]
    fn test_filter() {
        let c = Collection::new(vec![1, 2, 3, 4, 5]);
        let filtered = c.filter(|x| x % 2 == 0).cloned();
        assert_eq!(filtered.into_items(), vec![2, 4]);
    }

    #[test]
    fn test_reduce() {
        let c = Collection::new(vec![1, 2, 3, 4]);
        let sum = c.reduce(|a, b| a + b);
        assert_eq!(sum, Some(10));
    }

    #[test]
    fn test_reduce_empty() {
        let c: Collection<i32> = Collection::new(vec![]);
        assert_eq!(c.reduce(|a, b| a + b), None);
    }

    #[test]
    fn test_fold() {
        let c = Collection::new(vec![1, 2, 3]);
        let sum = c.fold(0, |acc, x| acc + x);
        assert_eq!(sum, 6);
    }

    #[test]
    fn test_sort() {
        let c = Collection::new(vec![3, 1, 2]).sort();
        assert_eq!(c.into_items(), vec![1, 2, 3]);
    }

    #[test]
    fn test_sort_by() {
        let c = Collection::new(vec!["c", "a", "b"]).sort_by(|a, b| a.cmp(b));
        assert_eq!(c.into_items(), vec!["a", "b", "c"]);
    }

    #[test]
    fn test_reverse() {
        let c = Collection::new(vec![1, 2, 3]).reverse();
        assert_eq!(c.into_items(), vec![3, 2, 1]);
    }

    #[test]
    fn test_take() {
        let c = Collection::new(vec![1, 2, 3, 4, 5]);
        let taken = c.take(3).cloned();
        assert_eq!(taken.into_items(), vec![1, 2, 3]);
    }

    #[test]
    fn test_skip() {
        let c = Collection::new(vec![1, 2, 3, 4]);
        let skipped = c.skip(2).cloned();
        assert_eq!(skipped.into_items(), vec![3, 4]);
    }

    #[test]
    fn test_unique() {
        let c = Collection::new(vec![1, 2, 2, 3, 3, 3]);
        let unique = c.unique(|x| *x).cloned();
        assert_eq!(unique.into_items(), vec![1, 2, 3]);
    }

    #[test]
    fn test_chunk() {
        let c = Collection::new(vec![1, 2, 3, 4, 5]);
        let chunks = c.chunk(2);
        assert_eq!(chunks.count(), 3);
    }

    #[test]
    fn test_each() {
        let c = Collection::new(vec![1, 2, 3]);
        let mut sum = 0;
        c.each(|x| sum += x);
        assert_eq!(sum, 6);
    }

    #[test]
    fn test_pluck() {
        let c = Collection::new(vec!["hello", "world"]);
        let lengths = c.pluck(|s| s.len());
        assert_eq!(lengths.into_items(), vec![5, 5]);
    }

    #[test]
    fn test_from_vec() {
        let c: Collection<i32> = vec![1, 2, 3].into();
        assert_eq!(c.count(), 3);
    }

    #[test]
    fn test_into_vec() {
        let c = Collection::new(vec![1, 2, 3]);
        let v: Vec<i32> = c.into();
        assert_eq!(v, vec![1, 2, 3]);
    }

    #[test]
    fn test_into_iterator() {
        let c = Collection::new(vec![1, 2, 3]);
        let sum: i32 = c.into_iter().sum();
        assert_eq!(sum, 6);
    }

    #[test]
    fn test_collect_fn() {
        let c = collect(1..=5);
        assert_eq!(c.count(), 5);
    }

    #[test]
    fn test_filter_chain() {
        let result = Collection::new(vec![1, 2, 3, 4, 5, 6])
            .filter(|x| x % 2 == 0)
            .cloned()
            .map(|x| x * 10);
        assert_eq!(result.into_items(), vec![20, 40, 60]);
    }

    #[test]
    fn test_to_json() {
        let c = Collection::new(vec!["a", "b", "c"]);
        let json = c.to_json();
        assert_eq!(json, serde_json::json!(["a", "b", "c"]));
    }

    #[test]
    fn test_filter_ref() {
        let strs = vec!["apple", "banana", "cherry"];
        let c = Collection::new(strs);
        let filtered = c.filter(|s| s.starts_with('a')).cloned();
        assert_eq!(filtered.into_items(), vec!["apple"]);
    }

    #[test]
    fn test_get_out_of_bounds() {
        let c = Collection::new(vec![1]);
        assert_eq!(c.get(1), None);
        assert_eq!(c.get(usize::MAX), None);
    }

    #[test]
    fn test_empty_take_skip() {
        let c: Collection<i32> = Collection::new(vec![]);
        assert_eq!(c.take(5).cloned().into_items(), Vec::<i32>::new());
        assert_eq!(c.skip(5).cloned().into_items(), Vec::<i32>::new());
    }

    #[test]
    fn test_sort_reverse_chain() {
        let c = Collection::new(vec![5, 2, 8, 1, 9]).sort().reverse();
        assert_eq!(c.into_items(), vec![9, 8, 5, 2, 1]);
    }

    #[test]
    fn test_collect_from_iterator() {
        let c = collect(vec!["a", "b", "c"]);
        assert_eq!(c.count(), 3);
        assert_eq!(c.first(), Some(&"a"));
        assert_eq!(c.last(), Some(&"c"));
    }

    #[test]
    fn test_map_into_items() {
        let c = Collection::new(vec![1, 2, 3]);
        let items = c.into_items();
        assert_eq!(items, vec![1, 2, 3]);
    }

    #[test]
    fn test_items_ref_does_not_consume() {
        let c = Collection::new(vec![1, 2, 3]);
        assert_eq!(c.items().len(), 3);
        assert_eq!(c.count(), 3); // still accessible
    }
}

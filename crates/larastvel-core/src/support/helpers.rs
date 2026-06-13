pub fn dd(value: impl std::fmt::Debug) -> ! {
    eprintln!("{:?}", value);
    panic!("Dumped and died");
}

pub fn dump(value: impl std::fmt::Debug) {
    eprintln!("{:?}", value);
}

pub fn tap<T, F>(value: T, callback: F) -> T
where
    F: FnOnce(&T),
{
    callback(&value);
    value
}

pub fn with<T, F, R>(value: T, callback: F) -> R
where
    F: FnOnce(T) -> R,
{
    callback(value)
}

pub fn value<T: Clone>(val: &Option<T>, default: T) -> T {
    val.clone().unwrap_or(default)
}

pub fn collect<T, I: IntoIterator<Item = T>>(iter: I) -> Vec<T> {
    iter.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dump_does_not_panic() {
        dump("hello");
    }

    #[test]
    fn test_tap_modifies_value() {
        let mut called = false;
        let val = tap(42, |&n| {
            assert_eq!(n, 42);
            called = true;
        });
        assert_eq!(val, 42);
        assert!(called);
    }

    #[test]
    fn test_with_invokes_callback() {
        let result = with(5, |n| n * 2);
        assert_eq!(result, 10);
    }

    #[test]
    fn test_value_with_some() {
        let val = value(&Some(10), 0);
        assert_eq!(val, 10);
    }

    #[test]
    fn test_value_with_none() {
        let val: i32 = value(&None, 99);
        assert_eq!(val, 99);
    }

    #[test]
    fn test_collect_from_iterator() {
        let v = collect(1..=5);
        assert_eq!(v, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_collect_from_vec() {
        let v = collect(vec!["a", "b", "c"]);
        assert_eq!(v, vec!["a", "b", "c"]);
    }
}

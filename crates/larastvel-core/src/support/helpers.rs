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

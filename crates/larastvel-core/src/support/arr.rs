use rand::seq::SliceRandom;
use serde_json::{Map, Value};
use std::collections::HashMap;

pub struct Arr;

impl Arr {
    pub fn wrap<T>(value: T) -> Vec<T> {
        vec![value]
    }

    pub fn first<T>(items: &[T]) -> Option<&T> {
        items.first()
    }

    pub fn last<T>(items: &[T]) -> Option<&T> {
        items.last()
    }

    pub fn random<T>(items: &[T]) -> Option<&T> {
        items.choose(&mut rand::rngs::OsRng)
    }

    pub fn random_count<T: Clone>(items: &[T], count: usize) -> Vec<T> {
        if items.is_empty() || count == 0 {
            return vec![];
        }
        let count = count.min(items.len());
        let mut indices: Vec<usize> = (0..items.len()).collect();
        indices.shuffle(&mut rand::rngs::OsRng);
        indices.truncate(count);
        indices.into_iter().map(|i| items[i].clone()).collect()
    }

    pub fn join<T: std::fmt::Display>(items: &[T], glue: &str) -> String {
        items
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(glue)
    }

    pub fn join_last<T: std::fmt::Display>(items: &[T], glue: &str, last_glue: &str) -> String {
        let strings: Vec<String> = items.iter().map(|i| i.to_string()).collect();
        match strings.len() {
            0 => String::new(),
            1 => strings[0].clone(),
            _ => {
                let mut result = strings[..strings.len() - 1].join(glue);
                result.push_str(last_glue);
                result.push_str(&strings[strings.len() - 1]);
                result
            }
        }
    }

    pub fn collapse<T>(items: Vec<Vec<T>>) -> Vec<T> {
        items.into_iter().flatten().collect()
    }

    pub fn cross_join<T: Clone>(arrays: &[Vec<T>]) -> Vec<Vec<T>> {
        if arrays.is_empty() {
            return vec![];
        }
        let mut result: Vec<Vec<T>> = arrays[0].iter().map(|x| vec![x.clone()]).collect();
        for array in &arrays[1..] {
            if array.is_empty() {
                return vec![];
            }
            let mut new_result = Vec::new();
            for existing in &result {
                for item in array {
                    let mut combined = existing.clone();
                    combined.push(item.clone());
                    new_result.push(combined);
                }
            }
            result = new_result;
        }
        result
    }

    pub fn shuffle<T>(items: &mut [T]) {
        items.shuffle(&mut rand::rngs::OsRng);
    }

    pub fn sort_recursive(value: &mut Value) {
        match value {
            Value::Array(arr) => {
                for item in arr.iter_mut() {
                    Self::sort_recursive(item);
                }
                arr.sort_by(cmp_value);
            }
            Value::Object(map) => {
                for (_, val) in map.iter_mut() {
                    Self::sort_recursive(val);
                }
            }
            _ => {}
        }
    }

    pub fn flatten(value: &Value) -> Vec<Value> {
        match value {
            Value::Array(arr) => {
                let mut result = Vec::new();
                for item in arr {
                    if item.is_array() {
                        result.extend(Self::flatten(item));
                    } else {
                        result.push(item.clone());
                    }
                }
                result
            }
            _ => vec![value.clone()],
        }
    }

    pub fn divide<K, V>(map: HashMap<K, V>) -> (Vec<K>, Vec<V>) {
        map.into_iter().unzip()
    }

    pub fn only(map: &Map<String, Value>, keys: &[&str]) -> Map<String, Value> {
        let mut result = Map::new();
        for key in keys {
            if let Some(val) = map.get(*key) {
                result.insert(key.to_string(), val.clone());
            }
        }
        result
    }

    pub fn except(map: &Map<String, Value>, keys: &[&str]) -> Map<String, Value> {
        let mut result = map.clone();
        for key in keys {
            result.remove(*key);
        }
        result
    }

    pub fn add(map: &mut Map<String, Value>, key: &str, value: Value) {
        if !map.contains_key(key) {
            map.insert(key.to_string(), value);
        }
    }

    pub fn prepend_keys_with(map: Map<String, Value>, prefix: &str) -> Map<String, Value> {
        map.into_iter()
            .map(|(k, v)| (format!("{}{}", prefix, k), v))
            .collect()
    }

    pub fn is_assoc(value: &Value) -> bool {
        value.is_object()
    }

    pub fn is_list(value: &Value) -> bool {
        value.is_array()
    }

    pub fn get<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
        if key.is_empty() {
            return Some(value);
        }
        let parts: Vec<&str> = key.split('.').collect();
        let mut current: &'a Value = value;
        for part in parts {
            current = match current {
                Value::Object(map) => map.get(part)?,
                Value::Array(arr) => {
                    let idx: usize = part.parse().ok()?;
                    arr.get(idx)?
                }
                _ => return None,
            };
        }
        Some(current)
    }

    pub fn set(value: &mut Value, key: &str, new_value: Value) {
        let parts: Vec<&str> = key.split('.').collect();
        if parts.is_empty() {
            return;
        }
        let mut current = value;
        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                match current {
                    Value::Object(map) => {
                        map.insert(part.to_string(), new_value);
                    }
                    Value::Array(arr) => {
                        if let Ok(idx) = part.parse::<usize>() {
                            if idx < arr.len() {
                                arr[idx] = new_value;
                            }
                        }
                    }
                    _ => {}
                }
                return;
            }
            match current {
                Value::Object(map) => {
                    if !map.contains_key(*part) {
                        map.insert(part.to_string(), Value::Object(Map::new()));
                    }
                    current = map.get_mut(*part).unwrap();
                }
                _ => return,
            }
        }
    }

    pub fn has(value: &Value, key: &str) -> bool {
        Self::get(value, key).is_some()
    }

    pub fn has_any(value: &Value, keys: &[&str]) -> bool {
        keys.iter().any(|k| Self::has(value, k))
    }

    pub fn forget(value: &mut Value, key: &str) {
        let parts: Vec<&str> = key.split('.').collect();
        if parts.is_empty() {
            return;
        }
        let mut current = value;
        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                match current {
                    Value::Object(map) => {
                        map.remove(*part);
                    }
                    Value::Array(arr) => {
                        if let Ok(idx) = part.parse::<usize>() {
                            if idx < arr.len() {
                                arr.remove(idx);
                            }
                        }
                    }
                    _ => {}
                }
            } else {
                match current {
                    Value::Object(map) => {
                        if let Some(next) = map.get_mut(*part) {
                            current = next;
                        } else {
                            return;
                        }
                    }
                    _ => return,
                }
            }
        }
    }

    pub fn pull(value: &mut Value, key: &str) -> Option<Value> {
        let parts: Vec<&str> = key.split('.').collect();
        if parts.is_empty() {
            return None;
        }
        let mut current = value;
        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                match current {
                    Value::Object(map) => return map.remove(*part),
                    Value::Array(arr) => {
                        if let Ok(idx) = part.parse::<usize>() {
                            if idx < arr.len() {
                                return Some(arr.remove(idx));
                            }
                        }
                    }
                    _ => return None,
                }
            } else {
                match current {
                    Value::Object(map) => {
                        if let Some(next) = map.get_mut(*part) {
                            current = next;
                        } else {
                            return None;
                        }
                    }
                    _ => return None,
                }
            }
        }
        None
    }

    pub fn dot(value: &Value, prepend: &str) -> Map<String, Value> {
        let mut result = Map::new();
        match value {
            Value::Object(map) => {
                for (key, val) in map {
                    let new_key = if prepend.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", prepend, key)
                    };
                    match val {
                        Value::Object(_) | Value::Array(_) => {
                            let nested = Self::dot(val, &new_key);
                            result.extend(nested);
                        }
                        _ => {
                            result.insert(new_key, val.clone());
                        }
                    }
                }
            }
            Value::Array(arr) => {
                for (i, val) in arr.iter().enumerate() {
                    let new_key = if prepend.is_empty() {
                        i.to_string()
                    } else {
                        format!("{}.{}", prepend, i)
                    };
                    match val {
                        Value::Object(_) | Value::Array(_) => {
                            let nested = Self::dot(val, &new_key);
                            result.extend(nested);
                        }
                        _ => {
                            result.insert(new_key, val.clone());
                        }
                    }
                }
            }
            _ => {
                result.insert(prepend.to_string(), value.clone());
            }
        }
        result
    }

    pub fn undot(map: &Map<String, Value>) -> Value {
        let mut result = Map::new();
        for (key, value) in map {
            let parts: Vec<&str> = key.split('.').collect();
            let mut current = &mut result;
            for (i, part) in parts.iter().enumerate() {
                if i == parts.len() - 1 {
                    current.insert(part.to_string(), value.clone());
                } else {
                    current = current
                        .entry(part.to_string())
                        .or_insert_with(|| Value::Object(Map::new()))
                        .as_object_mut()
                        .expect("non-object value at intermediate path segment");
                }
            }
        }
        let mut result = Value::Object(result);
        arrayify(&mut result);
        result
    }
}

fn arrayify(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for val in map.values_mut() {
                arrayify(val);
            }
            let count = map.len();
            if count > 0 && (0..count).all(|i| map.contains_key(&i.to_string())) {
                let arr: Vec<Value> = (0..count)
                    .map(|i| map.remove(&i.to_string()).unwrap_or(Value::Null))
                    .collect();
                *value = Value::Array(arr);
            }
        }
        Value::Array(arr) => {
            for val in arr.iter_mut() {
                arrayify(val);
            }
        }
        _ => {}
    }
}

fn variant_idx(v: &Value) -> u8 {
    match v {
        Value::Null => 0,
        Value::Bool(_) => 1,
        Value::Number(_) => 2,
        Value::String(_) => 3,
        Value::Array(_) => 4,
        Value::Object(_) => 5,
    }
}

fn cmp_value(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (a, b) {
        (Value::Null, Value::Null) => std::cmp::Ordering::Equal,
        (Value::Null, _) => std::cmp::Ordering::Less,
        (_, Value::Null) => std::cmp::Ordering::Greater,
        (Value::Bool(ab), Value::Bool(bb)) => ab.cmp(bb),
        (Value::Number(an), Value::Number(bn)) => an
            .as_f64()
            .partial_cmp(&bn.as_f64())
            .unwrap_or(std::cmp::Ordering::Equal),
        (Value::String(as_), Value::String(bs_)) => as_.cmp(bs_),
        (Value::Array(_), _) => std::cmp::Ordering::Greater,
        (_, Value::Array(_)) => std::cmp::Ordering::Less,
        (Value::Object(_), _) => std::cmp::Ordering::Greater,
        (_, Value::Object(_)) => std::cmp::Ordering::Less,
        _ => variant_idx(a).cmp(&variant_idx(b)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap() {
        assert_eq!(Arr::wrap(42), vec![42]);
        assert_eq!(Arr::wrap("hello"), vec!["hello"]);
    }

    #[test]
    fn test_first() {
        assert_eq!(Arr::first(&[1, 2, 3]), Some(&1));
        assert_eq!(Arr::first::<i32>(&[]), None);
    }

    #[test]
    fn test_last() {
        assert_eq!(Arr::last(&[1, 2, 3]), Some(&3));
        assert_eq!(Arr::last::<i32>(&[]), None);
    }

    #[test]
    fn test_random() {
        let items = [10, 20, 30];
        let r = Arr::random(&items);
        assert!(r.is_some());
        assert!(items.contains(r.unwrap()));
        assert_eq!(Arr::random::<i32>(&[]), None);
    }

    #[test]
    fn test_random_count() {
        let items = [1, 2, 3, 4, 5];
        let r = Arr::random_count(&items, 3);
        assert_eq!(r.len(), 3);
        for v in &r {
            assert!(items.contains(v));
        }
    }

    #[test]
    fn test_random_count_exceeds_length() {
        let items = [1, 2, 3];
        let r = Arr::random_count(&items, 10);
        assert_eq!(r.len(), 3);
    }

    #[test]
    fn test_random_count_empty() {
        assert_eq!(Arr::random_count::<i32>(&[], 3).len(), 0);
        assert_eq!(Arr::random_count(&[1, 2], 0).len(), 0);
    }

    #[test]
    fn test_join() {
        assert_eq!(Arr::join(&[1, 2, 3], ", "), "1, 2, 3");
        assert_eq!(Arr::join::<i32>(&[], ", "), "");
        assert_eq!(Arr::join(&[42], ", "), "42");
    }

    #[test]
    fn test_join_last() {
        assert_eq!(
            Arr::join_last(&["a", "b", "c"], ", ", " and "),
            "a, b and c"
        );
        assert_eq!(Arr::join_last(&["a", "b"], ", ", " and "), "a and b");
        assert_eq!(Arr::join_last(&["a"], ", ", " and "), "a");
        assert_eq!(Arr::join_last::<i32>(&[], ", ", " and "), "");
    }

    #[test]
    fn test_collapse() {
        let input = vec![vec![1, 2], vec![3], vec![4, 5, 6]];
        assert_eq!(Arr::collapse(input), vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn test_collapse_empty() {
        assert_eq!(Arr::collapse::<i32>(vec![]), Vec::<i32>::new());
        assert_eq!(Arr::collapse(vec![vec![1], vec![]]), vec![1]);
    }

    #[test]
    fn test_cross_join() {
        let result = Arr::cross_join(&[vec![1, 2], vec![10, 20]]);
        assert_eq!(result.len(), 4);
        assert_eq!(result[0], vec![1, 10]);
        assert_eq!(result[1], vec![1, 20]);
        assert_eq!(result[2], vec![2, 10]);
        assert_eq!(result[3], vec![2, 20]);
    }

    #[test]
    fn test_cross_join_single() {
        let result = Arr::cross_join(&[vec![1, 2, 3]]);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], vec![1]);
        assert_eq!(result[1], vec![2]);
        assert_eq!(result[2], vec![3]);
    }

    #[test]
    fn test_cross_join_empty() {
        let result = Arr::cross_join::<i32>(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_cross_join_with_empty_array() {
        let result = Arr::cross_join(&[vec![1, 2], vec![]]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_shuffle() {
        let mut items = [1, 2, 3, 4, 5];
        let original = items;
        Arr::shuffle(&mut items);
        let mut sorted = items;
        sorted.sort();
        assert_eq!(sorted, original);
    }

    #[test]
    fn test_shuffle_empty() {
        let mut items: [i32; 0] = [];
        Arr::shuffle(&mut items);
        assert!(items.is_empty());
    }

    #[test]
    fn test_sort_recursive_array() {
        let mut value = serde_json::json!([3, 1, 2]);
        Arr::sort_recursive(&mut value);
        assert_eq!(value, serde_json::json!([1, 2, 3]));
    }

    #[test]
    fn test_sort_recursive_object_keys() {
        let mut value = serde_json::json!({"z": 3, "a": 1, "m": 2});
        Arr::sort_recursive(&mut value);
        let obj = value.as_object().unwrap();
        let keys: Vec<&String> = obj.keys().collect();
        assert_eq!(keys, vec!["a", "m", "z"]);
    }

    #[test]
    fn test_sort_recursive_nested() {
        let mut value = serde_json::json!({"b": [3, 1, 2], "a": [6, 5]});
        Arr::sort_recursive(&mut value);
        let obj = value.as_object().unwrap();
        assert_eq!(obj["a"], serde_json::json!([5, 6]));
        assert_eq!(obj["b"], serde_json::json!([1, 2, 3]));
    }

    #[test]
    fn test_flatten() {
        let value = serde_json::json!([1, [2, 3], [4, [5, 6]]]);
        let flat = Arr::flatten(&value);
        assert_eq!(
            flat,
            vec![
                Value::from(1),
                Value::from(2),
                Value::from(3),
                Value::from(4),
                Value::from(5),
                Value::from(6),
            ]
        );
    }

    #[test]
    fn test_flatten_non_array() {
        let value = Value::from(42);
        assert_eq!(Arr::flatten(&value), vec![Value::from(42)]);
    }

    #[test]
    fn test_flatten_empty() {
        let value = serde_json::json!([]);
        let flat = Arr::flatten(&value);
        assert!(flat.is_empty());
    }

    #[test]
    fn test_divide() {
        let mut map = HashMap::new();
        map.insert("a", 1);
        map.insert("b", 2);
        let (keys, values) = Arr::divide(map);
        let mut k: Vec<_> = keys.into_iter().collect();
        let mut v: Vec<_> = values.into_iter().collect();
        k.sort();
        v.sort();
        assert_eq!(k, vec!["a", "b"]);
        assert_eq!(v, vec![1, 2]);
    }

    #[test]
    fn test_divide_empty() {
        let map: HashMap<String, i32> = HashMap::new();
        let (keys, values) = Arr::divide(map);
        assert!(keys.is_empty());
        assert!(values.is_empty());
    }

    #[test]
    fn test_only() {
        let map = serde_json::json!({"name": "John", "age": 30, "email": "john@test.com"});
        let map = map.as_object().unwrap();
        let result = Arr::only(map, &["name", "email"]);
        assert_eq!(result.len(), 2);
        assert!(result.contains_key("name"));
        assert!(result.contains_key("email"));
        assert!(!result.contains_key("age"));
    }

    #[test]
    fn test_only_missing_keys() {
        let map = serde_json::json!({"name": "John"});
        let map = map.as_object().unwrap();
        let result = Arr::only(map, &["name", "missing"]);
        assert_eq!(result.len(), 1);
        assert!(result.contains_key("name"));
    }

    #[test]
    fn test_except() {
        let map = serde_json::json!({"name": "John", "age": 30, "email": "john@test.com"});
        let map = map.as_object().unwrap();
        let result = Arr::except(map, &["age"]);
        assert_eq!(result.len(), 2);
        assert!(result.contains_key("name"));
        assert!(result.contains_key("email"));
        assert!(!result.contains_key("age"));
    }

    #[test]
    fn test_add_new_key() {
        let mut map = Map::new();
        Arr::add(&mut map, "key", Value::from("val"));
        assert_eq!(map.get("key").unwrap(), "val");
    }

    #[test]
    fn test_add_existing_key() {
        let mut map = Map::new();
        map.insert("key".to_string(), Value::from("original"));
        Arr::add(&mut map, "key", Value::from("new"));
        assert_eq!(map.get("key").unwrap(), "original");
    }

    #[test]
    fn test_prepend_keys_with() {
        let mut map = Map::new();
        map.insert("name".to_string(), Value::from("John"));
        map.insert("age".to_string(), Value::from(30));
        let result = Arr::prepend_keys_with(map, "user_");
        assert!(result.contains_key("user_name"));
        assert!(result.contains_key("user_age"));
        assert!(!result.contains_key("name"));
        assert_eq!(result.get("user_name").unwrap(), "John");
    }

    #[test]
    fn test_is_assoc() {
        assert!(Arr::is_assoc(&serde_json::json!({"key": "val"})));
        assert!(!Arr::is_assoc(&serde_json::json!([1, 2, 3])));
        assert!(!Arr::is_assoc(&Value::Null));
    }

    #[test]
    fn test_is_list() {
        assert!(Arr::is_list(&serde_json::json!([1, 2, 3])));
        assert!(!Arr::is_list(&serde_json::json!({"key": "val"})));
    }

    #[test]
    fn test_get_dot_notation() {
        let value = serde_json::json!({
            "user": {
                "name": "John",
                "address": {
                    "city": "New York"
                }
            }
        });
        assert_eq!(Arr::get(&value, "user.name").unwrap(), &Value::from("John"));
        assert_eq!(
            Arr::get(&value, "user.address.city").unwrap(),
            &Value::from("New York")
        );
    }

    #[test]
    fn test_get_array_index() {
        let value = serde_json::json!({"items": [10, 20, 30]});
        assert_eq!(Arr::get(&value, "items.0").unwrap(), &Value::from(10));
        assert_eq!(Arr::get(&value, "items.2").unwrap(), &Value::from(30));
    }

    #[test]
    fn test_get_missing_key() {
        let value = serde_json::json!({"name": "John"});
        assert_eq!(Arr::get(&value, "missing"), None);
        assert_eq!(Arr::get(&value, "name.missing"), None);
    }

    #[test]
    fn test_get_empty_path() {
        let value = serde_json::json!(42);
        assert_eq!(Arr::get(&value, ""), Some(&value));
    }

    #[test]
    fn test_set_dot_notation() {
        let mut value = serde_json::json!({"user": {"name": "John"}});
        Arr::set(&mut value, "user.name", Value::from("Jane"));
        assert_eq!(Arr::get(&value, "user.name").unwrap(), &Value::from("Jane"));
    }

    #[test]
    fn test_set_creates_intermediate() {
        let mut value = serde_json::json!({});
        Arr::set(&mut value, "user.name", Value::from("John"));
        assert_eq!(Arr::get(&value, "user.name").unwrap(), &Value::from("John"));
    }

    #[test]
    fn test_set_array_index() {
        let mut value = serde_json::json!({"items": [1, 2, 3]});
        Arr::set(&mut value, "items.1", Value::from(99));
        assert_eq!(Arr::get(&value, "items.1").unwrap(), &Value::from(99));
    }

    #[test]
    fn test_has() {
        let value = serde_json::json!({"user": {"name": "John"}});
        assert!(Arr::has(&value, "user"));
        assert!(Arr::has(&value, "user.name"));
        assert!(!Arr::has(&value, "user.age"));
        assert!(!Arr::has(&value, "missing"));
    }

    #[test]
    fn test_has_any() {
        let value = serde_json::json!({"name": "John", "age": 30});
        assert!(Arr::has_any(&value, &["name", "missing"]));
        assert!(!Arr::has_any(&value, &["foo", "bar"]));
    }

    #[test]
    fn test_forget() {
        let mut value = serde_json::json!({"user": {"name": "John", "age": 30}});
        Arr::forget(&mut value, "user.age");
        assert!(Arr::has(&value, "user.name"));
        assert!(!Arr::has(&value, "user.age"));
    }

    #[test]
    fn test_forget_array_index() {
        let mut value = serde_json::json!({"items": [10, 20, 30]});
        Arr::forget(&mut value, "items.1");
        assert_eq!(Arr::get(&value, "items.1").unwrap(), &Value::from(30));
    }

    #[test]
    fn test_forget_missing_key() {
        let mut value = serde_json::json!({"name": "John"});
        Arr::forget(&mut value, "missing.key");
        assert_eq!(value, serde_json::json!({"name": "John"}));
    }

    #[test]
    fn test_pull() {
        let mut value = serde_json::json!({"user": {"name": "John", "age": 30}});
        let pulled = Arr::pull(&mut value, "user.name");
        assert_eq!(pulled, Some(Value::from("John")));
        assert!(!Arr::has(&value, "user.name"));
        assert!(Arr::has(&value, "user.age"));
    }

    #[test]
    fn test_pull_missing() {
        let mut value = serde_json::json!({"name": "John"});
        assert_eq!(Arr::pull(&mut value, "missing"), None);
    }

    #[test]
    fn test_dot() {
        let value = serde_json::json!({
            "user": {
                "name": "John",
                "address": {
                    "city": "New York"
                }
            },
            "active": true
        });
        let dotted = Arr::dot(&value, "");
        assert_eq!(dotted.get("user.name").unwrap(), &Value::from("John"));
        assert_eq!(
            dotted.get("user.address.city").unwrap(),
            &Value::from("New York")
        );
        assert_eq!(dotted.get("active").unwrap(), &Value::Bool(true));
    }

    #[test]
    fn test_dot_with_array() {
        let value = serde_json::json!({"items": [10, 20, 30]});
        let dotted = Arr::dot(&value, "");
        assert_eq!(dotted.get("items.0").unwrap(), &Value::from(10));
        assert_eq!(dotted.get("items.2").unwrap(), &Value::from(30));
    }

    #[test]
    fn test_dot_scalar() {
        let value = Value::from(42);
        let dotted = Arr::dot(&value, "root");
        assert_eq!(dotted.get("root").unwrap(), &Value::from(42));
    }

    #[test]
    fn test_undot() {
        let mut map = Map::new();
        map.insert("user.name".to_string(), Value::from("John"));
        map.insert("user.age".to_string(), Value::from(30));
        map.insert("active".to_string(), Value::Bool(true));

        let result = Arr::undot(&map);
        assert_eq!(
            Arr::get(&result, "user.name").unwrap(),
            &Value::from("John")
        );
        assert_eq!(Arr::get(&result, "user.age").unwrap(), &Value::from(30));
        assert_eq!(Arr::get(&result, "active").unwrap(), &Value::Bool(true));
    }

    #[test]
    fn test_undot_roundtrip() {
        let original = serde_json::json!({
            "user": {
                "name": "John",
                "address": {
                    "city": "New York"
                }
            },
            "items": [1, 2, 3]
        });
        let dotted = Arr::dot(&original, "");
        let undotted = Arr::undot(&dotted);
        assert_eq!(original, undotted);
    }

    #[test]
    fn test_undot_empty() {
        let map = Map::new();
        let result = Arr::undot(&map);
        assert!(result.is_object());
        assert!(result.as_object().unwrap().is_empty());
    }
}

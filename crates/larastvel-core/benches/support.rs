use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;

fn bench_arr(c: &mut Criterion) {
    use larastvel_core::Arr;
    use serde_json::json;

    let numbers: Vec<i32> = (0..1000).collect();
    let strings: Vec<String> = (0..1000).map(|i| format!("item-{i}")).collect();

    c.bench_function("Arr::wrap", |b| b.iter(|| Arr::wrap(black_box("hello"))));

    c.bench_function("Arr::first", |b| b.iter(|| Arr::first(black_box(&numbers))));

    c.bench_function("Arr::last", |b| b.iter(|| Arr::last(black_box(&numbers))));

    c.bench_function("Arr::join", |b| {
        b.iter(|| Arr::join(black_box(&strings), black_box(", ")))
    });

    c.bench_function("Arr::collapse", |b| {
        let arr = vec![vec![1, 2], vec![3, 4], vec![5, 6]];
        b.iter(|| Arr::collapse(black_box(arr.clone())))
    });

    c.bench_function("Arr::divide", |b| {
        let map: std::collections::HashMap<&str, i32> =
            [("a", 1), ("b", 2), ("c", 3)].into_iter().collect();
        b.iter(|| Arr::divide(black_box(map.clone())))
    });

    c.bench_function("Arr::dot_nested", |b| {
        let value = json!({
            "user": {
                "profile": {
                    "name": "John",
                    "address": { "city": "NYC", "zip": "10001" }
                }
            }
        });
        b.iter(|| Arr::get(black_box(&value), black_box("user.profile.address.city")))
    });

    c.bench_function("Arr::set_nested", |b| {
        b.iter(|| {
            let mut value = json!({"a": {"b": {"c": 1}}});
            Arr::set(&mut value, "a.b.c", json!(42))
        })
    });

    c.bench_function("Arr::has", |b| {
        let value = json!({"user": {"profile": {"name": "John"}}});
        b.iter(|| Arr::has(black_box(&value), black_box("user.profile.name")))
    });

    c.bench_function("Arr::has_any", |b| {
        let value = json!({"user": {"profile": {"name": "John"}}});
        let keys = &["user.profile.name", "user.profile.age"];
        b.iter(|| Arr::has_any(black_box(&value), black_box(keys)))
    });

    c.bench_function("Arr::flatten", |b| {
        let value = json!({"a": 1, "b": [2, 3], "c": {"d": 4, "e": [5]}});
        b.iter(|| Arr::flatten(black_box(&value)))
    });

    c.bench_function("Arr::sort_recursive", |b| {
        b.iter(|| {
            let mut value = json!({"c": 3, "a": 1, "b": 2, "d": {"f": 6, "e": 5}});
            Arr::sort_recursive(&mut value)
        })
    });

    c.bench_function("Arr::is_assoc", |b| {
        let value = json!({"a": 1, "b": 2});
        b.iter(|| Arr::is_assoc(black_box(&value)))
    });

    c.bench_function("Arr::only", |b| {
        b.iter(|| {
            let map = json!({"a": 1, "b": 2, "c": 3, "d": 4, "e": 5});
            let map = map.as_object().unwrap().clone();
            Arr::only(&map, &["a", "c", "e"])
        })
    });

    c.bench_function("Arr::except", |b| {
        b.iter(|| {
            let map = json!({"a": 1, "b": 2, "c": 3, "d": 4, "e": 5});
            let map = map.as_object().unwrap().clone();
            Arr::except(&map, &["b", "d"])
        })
    });

    c.bench_function("Arr::dot", |b| {
        let value = json!({"user": {"profile": {"name": "John", "tags": ["dev", "rust"]}}});
        b.iter(|| Arr::dot(black_box(&value), black_box("")))
    });

    c.bench_function("Arr::forget", |b| {
        b.iter(|| {
            let mut value = json!({"user": {"profile": {"name": "John", "age": 30}}});
            Arr::forget(&mut value, "user.profile.age")
        })
    });

    c.bench_function("Arr::prepend_keys_with", |b| {
        b.iter(|| {
            let map = json!({"name": "John", "age": 30});
            let map = map.as_object().unwrap().clone();
            Arr::prepend_keys_with(map, "user.")
        })
    });
}

fn bench_collection(c: &mut Criterion) {
    use larastvel_core::Collection;

    let items: Vec<i32> = (0..1000).collect();
    let collection = Collection::new(items);

    c.bench_function("Collection::new", |b| {
        b.iter(|| Collection::new(black_box(vec![1, 2, 3, 4, 5])))
    });

    c.bench_function("Collection::map", |b| {
        b.iter(|| collection.clone().map(|x| black_box(x) * 2))
    });

    c.bench_function("Collection::filter", |b| {
        b.iter(|| {
            let col = collection.clone();
            let _ = col.filter(|x| black_box(x) % 2 == 0);
        })
    });

    c.bench_function("Collection::reduce", |b| {
        b.iter(|| {
            let col = collection.clone();
            let _ = col.into_iter().reduce(|acc, x| black_box(acc + x));
        })
    });

    c.bench_function("Collection::sort", |b| {
        let col = Collection::new((0..1000).rev().collect::<Vec<_>>());
        b.iter(|| col.clone().sort())
    });

    c.bench_function("Collection::reverse", |b| {
        b.iter(|| collection.clone().reverse())
    });

    c.bench_function("Collection::unique", |b| {
        let col = Collection::new(vec![1, 1, 2, 2, 3, 3, 4, 4, 5, 5]);
        b.iter(|| {
            let c = col.clone();
            let _ = c.unique(|x| *x);
        })
    });

    c.bench_function("Collection::chunk", |b| {
        b.iter(|| {
            let col = collection.clone();
            let _ = col.chunk(black_box(10));
        })
    });

    c.bench_function("Collection::take", |b| {
        b.iter(|| {
            let col = collection.clone();
            let _ = col.take(black_box(10));
        })
    });

    c.bench_function("Collection::skip", |b| {
        b.iter(|| {
            let col = collection.clone();
            let _ = col.skip(black_box(990));
        })
    });
}

fn bench_str(c: &mut Criterion) {
    use larastvel_core::Str;

    c.bench_function("Str::slug", |b| {
        b.iter(|| Str::slug(black_box("Hello World! This is a Test."), black_box("-")))
    });

    c.bench_function("Str::camel", |b| {
        b.iter(|| Str::camel(black_box("hello_world_rust_framework")))
    });

    c.bench_function("Str::studly", |b| {
        b.iter(|| Str::studly(black_box("hello_world_rust_framework")))
    });

    c.bench_function("Str::snake", |b| {
        b.iter(|| Str::snake(black_box("helloWorldRustFramework")))
    });

    c.bench_function("Str::kebab", |b| {
        b.iter(|| Str::kebab(black_box("helloWorldRustFramework")))
    });

    c.bench_function("Str::title", |b| {
        b.iter(|| Str::title(black_box("the quick brown fox jumps over the lazy dog")))
    });

    c.bench_function("Str::headline", |b| {
        b.iter(|| Str::headline(black_box("the_quick_brown_fox")))
    });

    c.bench_function("Str::contains", |b| {
        b.iter(|| {
            Str::contains(
                black_box("the quick brown fox jumps over the lazy dog"),
                black_box("fox"),
            )
        })
    });

    c.bench_function("Str::replace", |b| {
        b.iter(|| {
            Str::replace(
                black_box("the quick brown fox fox fox fox"),
                black_box("fox"),
                black_box("cat"),
            )
        })
    });

    c.bench_function("Str::mask", |b| {
        b.iter(|| {
            Str::mask(
                black_box("1234-5678-9012-3456"),
                black_box("*"),
                black_box(0),
                black_box(4),
            )
        })
    });

    c.bench_function("Str::pad_left", |b| {
        b.iter(|| Str::pad_left(black_box("hello"), black_box(10), black_box("*")))
    });

    c.bench_function("Str::pad_both", |b| {
        b.iter(|| Str::pad_both(black_box("hello"), black_box(11), black_box("*")))
    });

    c.bench_function("Str::random", |b| b.iter(|| Str::random(black_box(64))));

    c.bench_function("Str::random_numeric", |b| {
        b.iter(|| Str::random_numeric(black_box(16)))
    });

    c.bench_function("Str::between", |b| {
        b.iter(|| Str::between(black_box("[Hello] [World]"), black_box("["), black_box("]")))
    });
}

fn bench_datetime(c: &mut Criterion) {
    use larastvel_core::Dt;

    c.bench_function("Dt::now", |b| b.iter(Dt::now));

    c.bench_function("Dt::parse", |b| {
        b.iter(|| Dt::parse(black_box("2024-01-15 10:30:00")))
    });

    c.bench_function("Dt::format", |b| {
        let dt = Dt::now();
        b.iter(|| dt.format(black_box("%Y-%m-%d %H:%M:%S")))
    });

    c.bench_function("Dt::add_days", |b| {
        let dt = Dt::now();
        b.iter(|| dt.add_days(black_box(30)))
    });

    c.bench_function("Dt::sub_days", |b| {
        let dt = Dt::now();
        b.iter(|| dt.sub_days(black_box(30)))
    });

    c.bench_function("Dt::diff_in_days", |ben| {
        let a = Dt::now();
        let later = Dt::now().add_days(10);
        ben.iter(|| a.diff_in_days(black_box(&later)))
    });

    c.bench_function("Dt::start_of_day", |b| {
        let dt = Dt::now();
        b.iter(|| dt.start_of_day())
    });

    c.bench_function("Dt::end_of_month", |b| {
        let dt = Dt::now();
        b.iter(|| dt.end_of_month())
    });

    c.bench_function("Dt::timestamp", |b| {
        let dt = Dt::now();
        b.iter(|| dt.timestamp())
    });

    c.bench_function("Dt::is_weekend", |b| {
        let dt = Dt::now();
        b.iter(|| dt.is_weekend())
    });

    c.bench_function("Dt::from_ymd", |b| {
        b.iter(|| Dt::from_ymd(black_box(2024), black_box(3), black_box(15)))
    });
}

fn bench_number(c: &mut Criterion) {
    use larastvel_core::Number;

    c.bench_function("Number::format", |b| {
        b.iter(|| Number::format(black_box(1234567.89), black_box(2)))
    });

    c.bench_function("Number::percentage", |b| {
        b.iter(|| Number::percentage(black_box(50.0), black_box(200.0), black_box(2)))
    });

    c.bench_function("Number::ordinal", |b| {
        b.iter(|| Number::ordinal(black_box(123)))
    });

    c.bench_function("Number::file_size", |b| {
        b.iter(|| Number::file_size(black_box(1048576), black_box(2)))
    });

    c.bench_function("Number::abbreviate", |b| {
        b.iter(|| Number::abbreviate(black_box(2500000.0), black_box(2)))
    });

    c.bench_function("Number::currency", |b| {
        b.iter(|| Number::currency(black_box(1234.56), black_box("USD")))
    });

    c.bench_function("Number::for_humans", |b| {
        b.iter(|| Number::for_humans(black_box(2500000.0), black_box(2)))
    });

    c.bench_function("Number::round", |b| {
        b.iter(|| Number::round(black_box(5.456), black_box(2)))
    });

    c.bench_function("Number::clamp", |b| {
        b.iter(|| Number::clamp(black_box(5.0), black_box(1.0), black_box(10.0)))
    });
}

criterion_group!(
    benches,
    bench_arr,
    bench_collection,
    bench_str,
    bench_datetime,
    bench_number,
);
criterion_main!(benches);

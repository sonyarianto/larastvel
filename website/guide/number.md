# Number Helpers

Larastvel's `Number` provides a set of static helper methods for numeric formatting, ordinal generation, file size display, and more.

## Basic Formatting

```rust
use larastvel_core::Number;

Number::format(1234567.89, 2);  // "1,234,567.89"
Number::format(1000.0, 0);      // "1,000"
Number::format(-1234.5, 2);     // "-1,234.50"
```

## Rounding

```rust
Number::round(5.456, 2);  // 5.46
Number::floor(5.7);        // 5.0
Number::ceil(5.2);         // 6.0
Number::clamp(15.0, 1.0, 10.0);  // 10.0
```

## Percentages

```rust
Number::percentage(50.0, 200.0, 2);  // "25.00%"
Number::percentage(25.0, 100.0, 0);  // "25%"
```

## Ordinals

```rust
Number::ordinal(1);    // "1st"
Number::ordinal(2);    // "2nd"
Number::ordinal(3);    // "3rd"
Number::ordinal(11);   // "11th"
Number::ordinal(21);   // "21st"
Number::ordinal(101);  // "101st"
```

## File Size

```rust
Number::file_size(500, 2);         // "500.00 B"
Number::file_size(1024, 2);        // "1.00 KB"
Number::file_size(1536, 1);        // "1.5 KB"
Number::file_size(1048576, 2);     // "1.00 MB"
Number::file_size(1073741824, 2);  // "1.00 GB"
```

## Number Abbreviation

```rust
Number::abbreviate(2500.0, 1);          // "2.5K"
Number::abbreviate(2500000.0, 2);       // "2.50M"
Number::abbreviate(2500000000.0, 2);    // "2.50B"
Number::abbreviate(2500000000000.0, 2); // "2.50T"
Number::abbreviate(-2500.0, 1);         // "-2.5K"
```

## Currency

```rust
Number::currency(100.0, "USD");   // "$100.00"
Number::currency(50.5, "EUR");    // "€50.50"
Number::currency(25.0, "GBP");    // "£25.00"
Number::currency(1000.0, "JPY");  // "¥1000.00"
Number::currency(99.99, "INR");   // "₹99.99"
```

Supports: USD, EUR, GBP, JPY, CNY, KRW, INR, RUB, BRL, CHF, SEK, NOK, DKK, PLN, TRY, MXN, ZAR, PHP, MYR, THB, IDR, VND, AUD, CAD, NZD, SGD, HKD.

## Human Readable

```rust
Number::for_humans(500.0, 2);              // "500.00"
Number::for_humans(1500.0, 1);             // "1.5 thousand"
Number::for_humans(2500000.0, 2);          // "2.50 million"
Number::for_humans(3000000000.0, 2);       // "3.00 billion"
Number::for_humans(4000000000000.0, 2);    // "4.00 trillion"
```

## Available Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `format(num, precision)` | `String` | Format number with thousands separators |
| `round(num, precision)` | `f64` | Round to precision |
| `floor(num)` | `f64` | Round down |
| `ceil(num)` | `f64` | Round up |
| `clamp(num, min, max)` | `f64` | Clamp within range |
| `percentage(numerator, denominator, precision)` | `String` | Calculate percentage |
| `ordinal(num)` | `String` | Add ordinal suffix (1st, 2nd, 3rd) |
| `file_size(bytes, precision)` | `String` | Human-readable file size |
| `abbreviate(num, precision)` | `String` | Abbreviate with K/M/B/T |
| `currency(num, currency)` | `String` | Format as currency |
| `for_humans(num, precision)` | `String` | Human-readable large numbers |

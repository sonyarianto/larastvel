pub struct Number;

impl Number {
    pub fn format(num: f64, precision: usize) -> String {
        let abs = num.abs();
        let int_part = abs.trunc() as u64;
        let frac_part = ((abs - abs.trunc()) * 10u64.pow(precision as u32) as f64).round() as u64;
        let int_str = int_part.to_string();
        let mut result = String::new();
        for (count, c) in int_str.chars().rev().enumerate() {
            if count > 0 && count % 3 == 0 {
                result.push(',');
            }
            result.push(c);
        }
        let formatted = result.chars().rev().collect::<String>();

        if num.is_sign_negative() && (int_part > 0 || frac_part > 0) {
            if precision == 0 {
                format!("-{formatted}")
            } else {
                format!("-{formatted}.{:0width$}", frac_part, width = precision)
            }
        } else {
            if precision == 0 {
                formatted
            } else {
                format!("{formatted}.{:0width$}", frac_part, width = precision)
            }
        }
    }

    pub fn percentage(numerator: f64, denominator: f64, precision: usize) -> String {
        if denominator == 0.0 {
            return if precision == 0 {
                "0%".to_string()
            } else {
                format!("{:.*}%", precision, 0.0)
            };
        }
        let pct = (numerator / denominator) * 100.0;
        format!("{:.*}%", precision, pct)
    }

    pub fn ordinal(num: u64) -> String {
        let suffix = match num % 100 {
            11..=13 => "th",
            n => match n % 10 {
                1 => "st",
                2 => "nd",
                3 => "rd",
                _ => "th",
            },
        };
        format!("{num}{suffix}")
    }

    pub fn file_size(bytes: u64, precision: usize) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB", "EB"];
        if bytes == 0 {
            return format!("{:.*} {}", precision, 0.0, UNITS[0]);
        }
        let bytes_f = bytes as f64;
        let unit_idx = (bytes_f.log(1024.0).floor() as usize).min(UNITS.len() - 1);
        let value = bytes_f / 1024u64.pow(unit_idx as u32) as f64;
        format!("{:.*} {}", precision, value, UNITS[unit_idx])
    }

    pub fn abbreviate(num: f64, precision: usize) -> String {
        if num == 0.0 {
            return format!("{:.*}", precision, 0.0);
        }
        let abs = num.abs();
        let suffixes = &["", "K", "M", "B", "T", "Q"];
        let idx = (abs.log(1000.0).floor() as usize).min(suffixes.len() - 1);
        let divisor = 1000u64.pow(idx as u32) as f64;
        let value = num / divisor;
        format!("{:.*}{}", precision, value, suffixes[idx])
    }

    pub fn currency(num: f64, currency: &str) -> String {
        let symbol = match currency.to_uppercase().as_str() {
            "USD" | "US" | "$" => "$",
            "EUR" => "€",
            "GBP" => "£",
            "JPY" => "¥",
            "CNY" => "¥",
            "KRW" => "₩",
            "INR" => "₹",
            "RUB" => "₽",
            "BRL" => "R$",
            "AUD" | "CAD" | "NZD" | "SGD" | "HKD" => "$",
            "CHF" => "CHF",
            "SEK" => "kr",
            "NOK" => "kr",
            "DKK" => "kr",
            "PLN" => "zł",
            "TRY" => "₺",
            "MXN" => "Mex$",
            "ZAR" => "R",
            "PHP" => "₱",
            "MYR" => "RM",
            "THB" => "฿",
            "IDR" => "Rp",
            "VND" => "₫",
            _ => currency,
        };
        format!("{}{:.2}", symbol, num)
    }

    pub fn clamp(num: f64, min: f64, max: f64) -> f64 {
        num.clamp(min, max)
    }

    pub fn floor(num: f64) -> f64 {
        num.floor()
    }

    pub fn ceil(num: f64) -> f64 {
        num.ceil()
    }

    pub fn round(num: f64, precision: usize) -> f64 {
        let factor = 10u64.pow(precision as u32) as f64;
        (num * factor).round() / factor
    }

    pub fn for_humans(num: f64, precision: usize) -> String {
        let abs = num.abs();
        if abs >= 1_000_000_000_000.0 {
            let suffixes = &["", " trillion", " quadrillion", " quintillion"];
            let idx = (abs.log(1_000_000_000_000.0).floor() as usize).min(suffixes.len() - 1);
            let divisor = 1_000_000_000_000u64.pow(idx as u32) as f64;
            let value = num / divisor;
            return format!("{:.*}{}", precision, value, suffixes[idx]);
        }
        if abs >= 1_000_000_000.0 {
            return format!("{:.*} billion", precision, num / 1_000_000_000.0);
        }
        if abs >= 1_000_000.0 {
            return format!("{:.*} million", precision, num / 1_000_000.0);
        }
        if abs >= 1_000.0 {
            return format!("{:.*} thousand", precision, num / 1_000.0);
        }
        format!("{:.*}", precision, num)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format() {
        assert_eq!(Number::format(1234.5, 2), "1,234.50");
        assert_eq!(Number::format(1000.0, 0), "1,000");
        assert_eq!(Number::format(0.0, 2), "0.00");
        assert_eq!(Number::format(-1234.5, 2), "-1,234.50");
        assert_eq!(Number::format(1234567.89, 2), "1,234,567.89");
        assert_eq!(Number::format(1.0, 4), "1.0000");
    }

    #[test]
    fn test_format_no_precision() {
        assert_eq!(Number::format(42.0, 0), "42");
        assert_eq!(Number::format(999.0, 0), "999");
    }

    #[test]
    fn test_percentage() {
        assert_eq!(Number::percentage(50.0, 200.0, 2), "25.00%");
        assert_eq!(Number::percentage(25.0, 100.0, 0), "25%");
        assert_eq!(Number::percentage(1.0, 3.0, 2), "33.33%");
    }

    #[test]
    fn test_percentage_zero_denominator() {
        assert_eq!(Number::percentage(10.0, 0.0, 2), "0.00%");
    }

    #[test]
    fn test_ordinal() {
        assert_eq!(Number::ordinal(1), "1st");
        assert_eq!(Number::ordinal(2), "2nd");
        assert_eq!(Number::ordinal(3), "3rd");
        assert_eq!(Number::ordinal(4), "4th");
        assert_eq!(Number::ordinal(11), "11th");
        assert_eq!(Number::ordinal(12), "12th");
        assert_eq!(Number::ordinal(13), "13th");
        assert_eq!(Number::ordinal(21), "21st");
        assert_eq!(Number::ordinal(22), "22nd");
        assert_eq!(Number::ordinal(23), "23rd");
        assert_eq!(Number::ordinal(101), "101st");
        assert_eq!(Number::ordinal(111), "111th");
        assert_eq!(Number::ordinal(0), "0th");
    }

    #[test]
    fn test_file_size() {
        assert_eq!(Number::file_size(0, 2), "0.00 B");
        assert_eq!(Number::file_size(500, 2), "500.00 B");
        assert_eq!(Number::file_size(1024, 2), "1.00 KB");
        assert_eq!(Number::file_size(1536, 1), "1.5 KB");
        assert_eq!(Number::file_size(1048576, 2), "1.00 MB");
        assert_eq!(Number::file_size(1073741824, 2), "1.00 GB");
        assert_eq!(Number::file_size(1099511627776, 2), "1.00 TB");
    }

    #[test]
    fn test_abbreviate() {
        assert_eq!(Number::abbreviate(0.0, 2), "0.00");
        assert_eq!(Number::abbreviate(500.0, 2), "500.00");
        assert_eq!(Number::abbreviate(2500.0, 1), "2.5K");
        assert_eq!(Number::abbreviate(2500000.0, 2), "2.50M");
        assert_eq!(Number::abbreviate(2500000000.0, 2), "2.50B");
        assert_eq!(Number::abbreviate(2500000000000.0, 2), "2.50T");
        assert_eq!(Number::abbreviate(-2500.0, 1), "-2.5K");
    }

    #[test]
    fn test_currency() {
        assert_eq!(Number::currency(100.0, "USD"), "$100.00");
        assert_eq!(Number::currency(50.5, "EUR"), "€50.50");
        assert_eq!(Number::currency(25.0, "GBP"), "£25.00");
        assert_eq!(Number::currency(1000.0, "JPY"), "¥1000.00");
        assert_eq!(Number::currency(99.99, "INR"), "₹99.99");
    }

    #[test]
    fn test_currency_unknown() {
        assert_eq!(Number::currency(100.0, "XYZ"), "XYZ100.00");
    }

    #[test]
    fn test_clamp() {
        assert_eq!(Number::clamp(5.0, 1.0, 10.0), 5.0);
        assert_eq!(Number::clamp(0.0, 1.0, 10.0), 1.0);
        assert_eq!(Number::clamp(15.0, 1.0, 10.0), 10.0);
    }

    #[test]
    fn test_floor() {
        assert_eq!(Number::floor(5.7), 5.0);
        assert_eq!(Number::floor(-2.3), -3.0);
    }

    #[test]
    fn test_ceil() {
        assert_eq!(Number::ceil(5.2), 6.0);
        assert_eq!(Number::ceil(-2.3), -2.0);
    }

    #[test]
    fn test_round() {
        assert_eq!(Number::round(5.456, 2), 5.46);
        assert_eq!(Number::round(5.454, 2), 5.45);
        assert_eq!(Number::round(5.5, 0), 6.0);
    }

    #[test]
    fn test_for_humans() {
        assert_eq!(Number::for_humans(500.0, 2), "500.00");
        assert_eq!(Number::for_humans(1500.0, 1), "1.5 thousand");
        assert_eq!(Number::for_humans(2500000.0, 2), "2.50 million");
        assert_eq!(Number::for_humans(3000000000.0, 2), "3.00 billion");
        assert_eq!(Number::for_humans(4000000000000.0, 2), "4.00 trillion");
    }
}

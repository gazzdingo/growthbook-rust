//! JS value coercions used by condition evaluation (mongrule.ts). A missing
//! attribute is `Empty`, since JS `getPath` returns `null` for missing paths.

use std::cmp::Ordering;

use crate::extensions::truthy;
use crate::model_public::GrowthBookAttributeValue;

/// JS `String(value)`.
pub fn js_string(value: &GrowthBookAttributeValue) -> String {
    match value {
        GrowthBookAttributeValue::Empty => String::from("null"),
        GrowthBookAttributeValue::String(it) => it.clone(),
        GrowthBookAttributeValue::Int(it) => js_number_string(*it as f64),
        GrowthBookAttributeValue::Float(it) => js_number_string(*it),
        GrowthBookAttributeValue::Bool(it) => it.to_string(),
        // Array.prototype.join renders null elements as "".
        GrowthBookAttributeValue::Array(items) => items
            .iter()
            .map(|item| match item {
                GrowthBookAttributeValue::Empty => String::new(),
                it => js_string(it),
            })
            .collect::<Vec<_>>()
            .join(","),
        GrowthBookAttributeValue::Object(_) => String::from("[object Object]"),
    }
}

/// JS `Number(value)`; `NaN` when the value does not convert.
pub fn js_number(value: &GrowthBookAttributeValue) -> f64 {
    match to_primitive(value) {
        Primitive::Number(it) => it,
        Primitive::String(it) => js_string_to_number(&it),
    }
}

/// JS `JSON.stringify(value)`.
pub fn js_json(value: &GrowthBookAttributeValue) -> String {
    match value {
        GrowthBookAttributeValue::Empty => String::from("null"),
        GrowthBookAttributeValue::String(it) => serde_json::Value::from(it.as_str()).to_string(),
        GrowthBookAttributeValue::Int(it) => js_number_string(*it as f64),
        GrowthBookAttributeValue::Float(it) if it.is_finite() => js_number_string(*it),
        GrowthBookAttributeValue::Float(_) => String::from("null"),
        GrowthBookAttributeValue::Bool(it) => it.to_string(),
        GrowthBookAttributeValue::Array(items) => format!("[{}]", items.iter().map(js_json).collect::<Vec<_>>().join(",")),
        GrowthBookAttributeValue::Object(fields) => format!(
            "{{{}}}",
            fields
                .iter()
                .map(|field| format!("{}:{}", serde_json::Value::from(field.key.as_str()), js_json(&field.value)))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

/// JS non-operator condition (`evalConditionValue` for a plain value).
pub fn js_condition_equals(
    actual: &GrowthBookAttributeValue,
    expected: &GrowthBookAttributeValue,
    case_insensitive: bool,
) -> bool {
    match expected {
        GrowthBookAttributeValue::String(expected) if case_insensitive => js_string(actual).to_lowercase() == expected.to_lowercase(),
        GrowthBookAttributeValue::String(expected) => js_string(actual) == *expected,
        GrowthBookAttributeValue::Int(expected) => js_number(actual) == *expected as f64,
        GrowthBookAttributeValue::Float(expected) => js_number(actual) == *expected,
        GrowthBookAttributeValue::Bool(expected) => !matches!(actual, GrowthBookAttributeValue::Empty) && truthy(actual) == *expected,
        GrowthBookAttributeValue::Empty => matches!(actual, GrowthBookAttributeValue::Empty),
        GrowthBookAttributeValue::Array(_) | GrowthBookAttributeValue::Object(_) => js_json(actual) == js_json(expected),
    }
}

/// JS relational comparison (`<`, `>`): strings compare by UTF-16 code units,
/// everything else numerically. `None` when either side is `NaN`.
pub fn js_compare(
    actual: &GrowthBookAttributeValue,
    expected: &GrowthBookAttributeValue,
) -> Option<Ordering> {
    match (to_primitive(actual), to_primitive(expected)) {
        (Primitive::String(actual), Primitive::String(expected)) => Some(actual.encode_utf16().cmp(expected.encode_utf16())),
        (actual, expected) => actual.into_number().partial_cmp(&expected.into_number()),
    }
}

enum Primitive {
    Number(f64),
    String(String),
}

impl Primitive {
    fn into_number(self) -> f64 {
        match self {
            Primitive::Number(it) => it,
            Primitive::String(it) => js_string_to_number(&it),
        }
    }
}

fn to_primitive(value: &GrowthBookAttributeValue) -> Primitive {
    match value {
        GrowthBookAttributeValue::Empty => Primitive::Number(0.0),
        GrowthBookAttributeValue::Bool(it) => Primitive::Number(if *it { 1.0 } else { 0.0 }),
        GrowthBookAttributeValue::Int(it) => Primitive::Number(*it as f64),
        GrowthBookAttributeValue::Float(it) => Primitive::Number(*it),
        it => Primitive::String(js_string(it)),
    }
}

/// JS `StringToNumber`.
fn js_string_to_number(value: &str) -> f64 {
    let trimmed = value.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    if trimmed.is_empty() {
        return 0.0;
    }
    for (prefix, radix) in [("0x", 16), ("0X", 16), ("0o", 8), ("0O", 8), ("0b", 2), ("0B", 2)] {
        if let Some(digits) = trimmed.strip_prefix(prefix) {
            if digits.is_empty() {
                return f64::NAN;
            }
            return digits
                .chars()
                .try_fold(0.0, |acc, c| c.to_digit(radix).map(|digit| acc * radix as f64 + digit as f64))
                .unwrap_or(f64::NAN);
        }
    }
    let unsigned = trimmed.strip_prefix(['+', '-']).unwrap_or(trimmed);
    if unsigned == "Infinity" {
        return if trimmed.starts_with('-') { f64::NEG_INFINITY } else { f64::INFINITY };
    }
    if is_decimal_literal(unsigned) {
        trimmed.parse().unwrap_or(f64::NAN)
    } else {
        f64::NAN
    }
}

/// `digits[.digits][e[+-]digits]` or `.digits[e[+-]digits]`, as JS accepts.
fn is_decimal_literal(value: &str) -> bool {
    let (mantissa, exponent) = match value.find(['e', 'E']) {
        Some(index) => (&value[..index], Some(&value[index + 1..])),
        None => (value, None),
    };
    let (integer, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = |part: &str| part.chars().all(|c| c.is_ascii_digit());
    let mantissa_ok = digits(integer) && digits(fraction) && !(integer.is_empty() && fraction.is_empty());
    let exponent_ok = exponent.map_or(true, |exponent| {
        let exponent = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
        !exponent.is_empty() && digits(exponent)
    });
    mantissa_ok && exponent_ok
}

/// JS `Number.prototype.toString()` for a double.
fn js_number_string(value: f64) -> String {
    if value.is_nan() {
        return String::from("NaN");
    }
    if value.is_infinite() {
        return String::from(if value > 0.0 { "Infinity" } else { "-Infinity" });
    }
    if value == 0.0 {
        return String::from("0");
    }
    let sign = if value < 0.0 { "-" } else { "" };
    // Rust's shortest round-trip digits, e.g. "1.5e0" -> digits "15", n = 1.
    let scientific = format!("{:e}", value.abs());
    let (mantissa, exponent) = scientific.split_once('e').unwrap_or((&scientific, "0"));
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i32;
    let n = exponent.parse::<i32>().unwrap_or(0) + 1;
    let body = if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let exponent = n - 1;
        let exponent_sign = if exponent < 0 { "-" } else { "+" };
        let mantissa = if k == 1 { digits.clone() } else { format!("{}.{}", &digits[..1], &digits[1..]) };
        format!("{mantissa}e{exponent_sign}{}", exponent.abs())
    };
    format!("{sign}{body}")
}

#[cfg(test)]
mod test {
    use super::{js_number_string, js_string_to_number};

    #[test]
    fn number_to_string_matches_js() {
        for (value, expected) in [
            (1.0, "1"),
            (1.5, "1.5"),
            (-0.25, "-0.25"),
            (1e21, "1e+21"),
            (123456789012345680000.0, "123456789012345680000"),
            (1e-7, "1e-7"),
            (0.000001, "0.000001"),
            (1.5e-10, "1.5e-10"),
            (0.1 + 0.2, "0.30000000000000004"),
        ] {
            assert_eq!(js_number_string(value), expected, "{value}");
        }
    }

    #[test]
    fn string_to_number_matches_js() {
        for (value, expected) in [
            ("", 0.0),
            ("  12 ", 12.0),
            ("1.5", 1.5),
            (".5", 0.5),
            ("5.", 5.0),
            ("-1e3", -1000.0),
            ("0x1A", 26.0),
            ("+Infinity", f64::INFINITY),
        ] {
            assert_eq!(js_string_to_number(value), expected, "{value}");
        }
        for value in ["abc", "1.2.3", "2023-01-01", "inf", "NaN", "infinity", "0x", "-0x1", "1e", "."] {
            assert!(js_string_to_number(value).is_nan(), "{value}");
        }
    }
}

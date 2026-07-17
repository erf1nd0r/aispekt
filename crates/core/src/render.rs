//! Formatting helpers that must reproduce JS semantics exactly — the JSON
//! report is byte-compared against the TS engine's output.

/// JS `Number.prototype.toFixed(1)`: nearest multiple of 0.1, ties upward.
pub fn to_fixed1(x: f64) -> String {
    let n = (x * 10.0 + 0.5).floor() as i64;
    format!("{}.{}", n / 10, (n % 10).abs())
}

/// JS `Number.prototype.toLocaleString()` for integers (en-US grouping).
pub fn to_locale_string(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    let len = digits.len();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// JS `Math.round`: half rounds toward +Infinity. NOT `(x+0.5).floor()` —
/// that rounds up for values one ULP below n+0.5 (the
/// `Math.round(0.49999999999999994) === 0` class), where JS rounds down.
pub fn js_round(x: f64) -> f64 {
    let f = x.floor();
    if x - f >= 0.5 {
        f + 1.0
    } else {
        f
    }
}

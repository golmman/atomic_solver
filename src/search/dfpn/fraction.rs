//! Exact `f64` → reduced `num/den` conversion shared by ε arithmetic and
//! restart-schedule growth.

/// Convert a positive f64 into an exact reduced `num/den` fraction.
pub(super) fn fraction_from_f64(v: f64) -> (u64, u64) {
    let bits = v.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let mantissa = bits & 0x000f_ffff_ffff_ffff;
    let mut num = (1u64 << 52) | mantissa;
    let mut den = 1u64;

    let exp = exponent - 1075; // 1023 (bias) + 52 (fraction bits)
    if exp >= 0 {
        num <<= exp as u32;
    } else {
        den = 1u64 << (-exp) as u32;
    }

    let g = gcd(num, den);
    (num / g, den / g)
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

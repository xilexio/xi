#[cfg(not(test))]
pub fn random() -> f64 {
    js_sys::Math::random()
}

#[cfg(test)]
pub fn random() -> f64 {
    rand::random::<f64>()
}
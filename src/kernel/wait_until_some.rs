use crate::kernel::sleep::sleep;

/// Actively waits until given value is not `None` and then returns it.
pub async fn wait_until_some<T, F>(mut f: F) -> T
where
    F: FnMut() -> Option<T>,
{
    loop {
        if let Some(value) = f() {
            return value;
        } else {
            sleep(1).await;
        }
    }
}
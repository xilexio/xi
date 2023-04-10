pub fn upper_bound<T>(vec: &Vec<T>, searched: T) -> usize
where
    T: Copy + Ord,
{
    upper_bound_by_key(vec, searched, |&x| x)
}

#[inline]
pub fn upper_bound_by_key<T, F, R>(vec: &Vec<T>, searched: R, mut key: F) -> usize
where
    F: FnMut(&T) -> R,
    R: Ord,
{
    let mut i = 0;
    let mut j = vec.len();
    while i < j {
        let m = (i + j) / 2;
        if key(&vec[m]) <= searched {
            i = m + 1;
        } else {
            j = m;
        }
    }
    i
}

pub fn lower_bound<T>(vec: &Vec<T>, searched: T) -> usize
where
    T: Copy + Ord,
{
    lower_bound_by_key(vec, searched, |&x| x)
}

#[inline]
pub fn lower_bound_by_key<T, F, R>(vec: &Vec<T>, searched: R, mut key: F) -> usize
where
    F: FnMut(&T) -> R,
    R: Ord,
{
    let mut i = 0;
    let mut j = vec.len();
    while i < j {
        let m = (i + j) / 2;
        if key(&vec[m]) < searched {
            i = m + 1;
        } else {
            j = m;
        }
    }
    i
}

#[cfg(test)]
mod tests {
    use crate::algorithms::binary_search::{lower_bound, upper_bound};

    #[test]
    fn test_upper_bound() {
        assert_eq!(upper_bound(&vec![1, 2, 3], 2), 2);
        assert_eq!(upper_bound(&vec![1, 3, 5], 2), 1);
        assert_eq!(upper_bound(&vec![1], 0), 0);
        assert_eq!(upper_bound(&vec![1], 1), 1);
        assert_eq!(upper_bound(&vec![1], 2), 1);
    }

    #[test]
    fn test_lower_bound() {
        assert_eq!(lower_bound(&vec![1, 2, 3], 2), 1);
        assert_eq!(lower_bound(&vec![1, 3, 5], 2), 1);
        assert_eq!(lower_bound(&vec![1], 0), 0);
        assert_eq!(lower_bound(&vec![1], 1), 0);
        assert_eq!(lower_bound(&vec![1], 2), 1);
    }
}

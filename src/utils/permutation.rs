pub fn reverse_permutation(permutation: &[usize]) -> Vec<usize> {
    let mut result = vec![0; permutation.len()];
    for i in 0..permutation.len() {
        result[permutation[i]] = i;
    }
    result
}
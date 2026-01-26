/// Finds the maximum value in a vector of integers.
///
/// # Arguments
///
/// * `values` - A reference to a vector containing integer values.
///
/// # Returns
///
/// * `Option<i32>` - The maximum value wrapped in `Some`, or `None` if the vector is empty.
pub fn max_in_vector(values: &Vec<i32>) -> Option<i32> {
    // Use iterator's max method which returns an Option<&i32>
    values.iter().max().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_non_empty() {
        let v = vec![3, 1, 4, 1, 5];
        assert_eq!(max_in_vector(&v), Some(5));
    }

    #[test]
    fn test_single_element() {
        let v = vec![-10];
        assert_eq!(max_in_vector(&v), Some(-10));
    }

    #[test]
    fn test_empty() {
        let v: Vec<i32> = Vec::new();
        assert_eq!(max_in_vector(&v), None);
    }
}

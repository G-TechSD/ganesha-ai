#[cfg(test)]
mod integration {
    #[test]
    fn test_greeting() {
        assert_eq!(super::greet(), "Hello, world!");
    }
}

pub fn add<T: std::ops::Add<Output = T>>(a: T, b: T) -> T {
    a + b
}

fn main() {
    let sum = add(3, 4);
    assert_eq!(sum, 7);
    println!("{} + {} = {}", 3, 4, sum);
}

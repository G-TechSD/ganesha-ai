mod utils;            // brings in utils.rs

fn main() {
    let sum = add(3, 4);
    assert_eq!(sum, 7);
    println!("{} + {} = {}", 3, 4, sum);

    let prod = utils::multiply(6, 7);
    println!("{} * {} = {}", 6, 7, prod);
}

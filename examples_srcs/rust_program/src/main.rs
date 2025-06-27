use std::fs;
use std::io::Write;

fn main() {
    let mut file = fs::File::create("D:\\pe_works.txt").unwrap();
    file.write(b"PE injection works").unwrap();
}

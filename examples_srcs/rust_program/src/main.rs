use std::fs;
use std::env;
use std::io::Write;

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut file = fs::File::create("D:\\pe_works.txt").unwrap();
    writeln!(file, "PE injection works").unwrap();
    writeln!(file, "Current directory: {:?}", env::current_dir().unwrap()).unwrap();

    for (idx, arg) in args.iter().enumerate() {
        writeln!(file, "Arg {idx}: {arg}").unwrap();
    }

    if args.len() == 0 {
        writeln!(file, "No args passed").unwrap();
    }
}

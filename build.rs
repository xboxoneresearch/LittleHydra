fn main() {
    println!("cargo::rerun-if-changed=.git/HEAD");
    println!("cargo:rustc-env=BUILD_DATE={}", chrono::Utc::now().to_rfc3339());
}
// Rebuild when a migration file changes: the migrations are embedded in the binary.
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}

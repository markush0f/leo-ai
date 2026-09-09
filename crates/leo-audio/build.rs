fn main() {
    // Runtime libs exist as versioned sonames; the unversioned -devel
    // symlinks are not installed, so we link the .so.N files directly.
    println!("cargo:rustc-link-search=native=/usr/lib64");
    println!("cargo:rustc-link-lib=dylib:+verbatim=libpulse-simple.so.0");
    println!("cargo:rustc-link-lib=dylib:+verbatim=libpulse.so.0");
}

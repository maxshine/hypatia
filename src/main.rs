fn main() {
    if let Err(e) = hypatia::cli::run() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

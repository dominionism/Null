fn main() {
    // Tauri takes Oh-my-pi into the app from Engine/ (tauri.conf.json), and when it is not
    // there says only that a path does not exist. Say what puts it there.
    let engine = format!("Engine/omp-{}", std::env::var("TARGET").unwrap_or_default());
    if !std::path::Path::new(&engine).exists() {
        eprintln!("{engine} is not there. Run Mini/Scripts/engine once: it fetches the Oh-my-pi that Null carries.");
        std::process::exit(1);
    }
    tauri_build::build()
}

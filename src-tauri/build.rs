fn main() {
    // Tauri only re-copies bundle resources when a *known* resource changed or
    // the build script re-ran. A file that appears inside an already-known
    // folder — which is exactly what `npm run vendor` produces — is otherwise
    // ignored in `tauri dev`. Watching the folder is the documented way out.
    println!("cargo:rerun-if-changed=resources");
    tauri_build::build()
}

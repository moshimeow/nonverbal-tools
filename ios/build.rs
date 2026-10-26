fn main() {
    println!("cargo:rerun-if-changed=src/audio_session.m");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("ios") {
        cc::Build::new()
            .file("src/audio_session.m")
            .flag("-fobjc-arc")
            .compile("speak_audio_session");
        println!("cargo:rustc-link-lib=framework=AVFAudio");
        println!("cargo:rustc-link-lib=framework=Foundation");
    }
}

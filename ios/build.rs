fn main() {
    println!("cargo:rerun-if-changed=src/platform.m");
    cc::Build::new()
        .file("src/platform.m")
        .flag("-fobjc-arc")
        .compile("speak_platform");
    println!("cargo:rustc-link-lib=framework=Foundation");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("ios") {
        println!("cargo:rustc-link-lib=framework=UIKit");
    } else {
        println!("cargo:rustc-link-lib=framework=AppKit");
    }

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

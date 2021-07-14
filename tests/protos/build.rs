use std::{env, path::PathBuf};

fn main() {
    // test protos
    tonic_build::configure()
        .compile(&["service.proto"], &["."])
        .unwrap();
}

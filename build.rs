use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use anyhow::{Context, Result};
use protox::prost::Message;
use protox::prost_reflect::prost_types::FileDescriptorSet;

macro_rules! rerun_if_changed {
    ($($path:expr),+ $(,)?) => {
        $(println!("cargo:rerun-if-changed={}", $path);)+
    };
}

const PROTO_DIR: &str = "proto/";
const PROTO_FILE_DIFF: &str = "proto/file_diff.proto";
const OUT_DIR_RUST: &str = "src/proto";
const DESCRIPTOR_SET_FILENAME: &str = "file_descriptor_set.bin";
const PROTO_MODULE_CODE: &str = "pub mod file_diff;\npub use file_diff::*;\n";

fn compile_proto_descriptors() -> Result<FileDescriptorSet> {
    protox::compile(&[PROTO_FILE_DIFF], &[PROTO_DIR]).context("failed to compile proto descriptors")
}

fn write_descriptor_set(fds: &FileDescriptorSet) -> Result<PathBuf> {
    let path: PathBuf = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR not set"))
        .join(DESCRIPTOR_SET_FILENAME);
    fs::write(&path, fds.encode_to_vec()).context("failed to write descriptor set")?;
    Ok(path)
}

fn generate_rust_code(descriptor_path: &Path) -> Result<()> {
    std::fs::create_dir_all(OUT_DIR_RUST).context("failed to create output directory")?;
    prost_build::Config::new()
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .type_attribute(".", "#[serde(rename_all = \"PascalCase\")]")
        .file_descriptor_set_path(descriptor_path)
        .skip_protoc_run()
        .out_dir(OUT_DIR_RUST)
        .compile_protos(&[PROTO_FILE_DIFF], &[PROTO_DIR])
        .context("failed to compile proto files")?;

    Ok(())
}

fn write_proto_mod_file() -> Result<()> {
    fs::write(Path::new(OUT_DIR_RUST).join("mod.rs"), PROTO_MODULE_CODE)
        .context("failed to write mod.rs")
}

fn main() -> Result<()> {
    rerun_if_changed!(PROTO_FILE_DIFF, "build.rs");

    let fds: FileDescriptorSet = compile_proto_descriptors()?;
    let descriptor_path: PathBuf = write_descriptor_set(&fds)?;
    generate_rust_code(&descriptor_path)?;
    write_proto_mod_file()?;

    Ok(())
}

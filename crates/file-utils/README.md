```rust
use file_handler::FileHandler;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
struct Entry {
    file_name: String,
    version: i64,
}

fn main() {
    env_logger::init();

    let map = HashMap::from([(
        "lua.arcx".into(),
        Entry { file_name: "lua.arcx".into(), version: 124 },
    )]);

    FileHandler::write_json(&map, "./out/data.json").unwrap();
    FileHandler::write_json_compact(&map, "./out/data.min.json").unwrap();
    FileHandler::write_json_atomic(&map, "./out/data.json").unwrap();
    FileHandler::write_text("hello world", "./out/hello.txt").unwrap();
    FileHandler::write_bytes(&[0xDE, 0xAD], "./out/blob.bin").unwrap();
    FileHandler::append_line("new log entry", "./out/app.log").unwrap();

    let text: Option<String> = FileHandler::read_text("./out/hello.txt");
    let bytes: Option<Vec<u8>> = FileHandler::read_bytes("./out/blob.bin");
    let json: Option<HashMap<String, Entry>> = FileHandler::read_json("./out/data.json");
    let lines: Option<Vec<String>> = FileHandler::read_lines("./out/app.log");

    FileHandler::ensure_dir("./cache/deep/nested").unwrap();
    println!("exists: {}", FileHandler::exists("./out/data.json"));
    println!("size:   {:?}", FileHandler::file_size("./out/blob.bin"));

    let files = FileHandler::list_dir("./out");
    let all   = FileHandler::walk_files("./out");

    FileHandler::copy_file("./out/hello.txt", "./backup/hello.txt").unwrap();
    FileHandler::move_file("./out/blob.bin", "./backup/blob.bin").unwrap();
    FileHandler::remove_file("./backup/hello.txt").unwrap();

    let md5    = FileHandler::compute_md5("./out/data.json");
    let sha256 = FileHandler::compute_sha256("./out/data.json");
    let ok     = FileHandler::verify_md5("./out/data.json", "d41d8cd98f00b204e9800998ecf8427e");

    FileHandler::verify_hash_with_log("kr", "./out/data.json", "abc123", Some("data.json"));
}
```
//! Generate `backend/config/openapi.yaml` from the code.

use std::fs;
use std::path::PathBuf;

use alice_daemon::server::openapi::spec;

fn main() {
    let spec = spec();
    let yaml = serde_yaml::to_string(&spec).expect("yaml serialization succeeds");
    let json = serde_json::to_string_pretty(&spec).expect("json serialization succeeds");

    // Write yaml to backend/config/openapi.yaml
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let yaml_path = manifest_dir.join("../../config/openapi.yaml");
    let json_path = manifest_dir.join("../../config/openapi.json");

    fs::create_dir_all(yaml_path.parent().expect("config dir has parent"))
        .expect("create config dir");
    fs::write(&yaml_path, &yaml).expect("write yaml");
    fs::write(&json_path, &json).expect("write json");

    println!("wrote {}", yaml_path.display());
    println!("wrote {}", json_path.display());
}

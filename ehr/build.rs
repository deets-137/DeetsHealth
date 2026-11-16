//! Generates the gRPC code from `proto/deets/ehr/v1/ehr.proto` (EHR-SERVICE.md §3), and makes
//! the PHI tag real in Rust (§4).
//!
//! 1. Google's own `protoc`, pinned by SHA-256 in `ehr/protoc.json`, writes the contract's
//!    descriptors. `scripts/protoc.mjs` puts it in `/.protoc/<version>/`; `npm run dev` and
//!    `npm run check` run that first. `PROTOC` and `PROTOC_INCLUDE` name another copy (a deploy
//!    image's). Either way its version must be the pinned one (DEPENDENCIES.md §5).
//! 2. This file walks the descriptor bytes itself and lists every field marked `(phi)` or
//!    `(secret)`. (`prost-types` drops unknown extensions, so it cannot see the marks.)
//! 3. Each message with a marked field loses `prost`'s derived `Debug`. `redacted.rs` gives it
//!    one that prints `***` for the marked fields, plus the list and a test that checks it.

use std::{env, fmt::Write as _, fs, path::PathBuf, process::Command};

use prost::Message as _;

const PROTO: &str = "proto/deets/ehr/v1/ehr.proto";
const PACKAGE: &str = "deets.ehr.v1";
const PHI: u64 = 50001;
const SECRET: u64 = 50002;
/// `FieldDescriptorProto.type` for a string.
const TYPE_STRING: u64 = 9;
/// FieldDescriptorProto.Label: LABEL_REPEATED.
const LABEL_REPEATED: u64 = 3;

fn main() {
    println!("cargo:rerun-if-changed=proto");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let (protoc, wkt) = pinned_protoc();
    let fds_path = out.join("ehr.fds");

    let status = Command::new(&protoc)
        .arg("--include_imports")
        .arg(format!("--descriptor_set_out={}", fds_path.display()))
        .arg("-Iproto")
        .arg(format!("-I{}", wkt.display()))
        .arg(PROTO)
        .status()
        .expect("run protoc");
    assert!(status.success(), "protoc failed on {PROTO}");

    let bytes = fs::read(&fds_path).unwrap();
    let messages = marked_messages(&bytes);

    let fds = prost_types::FileDescriptorSet::decode(bytes.as_slice()).unwrap();
    tonic_prost_build::configure()
        .skip_debug(messages.iter().map(|m| format!(".{PACKAGE}.{}", m.name)))
        .compile_fds(fds)
        .unwrap();

    fs::write(out.join("redacted.rs"), redacted_rs(&messages)).unwrap();
}

/// The pinned `protoc` and its include folder (the well-known types). Stops the build with the
/// command to run when it is missing, and when its version is not the pinned one.
fn pinned_protoc() -> (PathBuf, PathBuf) {
    println!("cargo:rerun-if-changed=protoc.json");
    println!("cargo:rerun-if-env-changed=PROTOC");
    println!("cargo:rerun-if-env-changed=PROTOC_INCLUDE");
    let pin: serde_json::Value = serde_json::from_str(&fs::read_to_string("protoc.json").unwrap()).unwrap();
    let version = pin["version"].as_str().expect("protoc.json: version");

    let (protoc, include) = match env::var_os("PROTOC") {
        Some(p) => {
            let include = env::var_os("PROTOC_INCLUDE").expect("PROTOC is set: set PROTOC_INCLUDE to its include folder too");
            (PathBuf::from(p), PathBuf::from(include))
        }
        None => {
            let dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("..").join(".protoc").join(version);
            let exe = if cfg!(windows) { "protoc.exe" } else { "protoc" };
            (dir.join("bin").join(exe), dir.join("include"))
        }
    };
    println!("cargo:rerun-if-changed={}", protoc.display());
    assert!(
        protoc.exists(),
        "protoc {version} is not installed at {}. Run once: node scripts/protoc.mjs",
        protoc.display()
    );
    let said = Command::new(&protoc).arg("--version").output().expect("run protoc --version");
    let said = String::from_utf8_lossy(&said.stdout);
    assert_eq!(
        said.trim(),
        format!("libprotoc {version}"),
        "{} is not the pinned protoc {version} (ehr/protoc.json)",
        protoc.display()
    );
    (protoc, include)
}

/// One message of the contract, with the fields the marks name.
struct Marked {
    name: String,
    /// Every field, in order: (name, marked, repeated).
    fields: Vec<(String, bool, bool)>,
}

/// The messages in our package that have at least one marked field.
fn marked_messages(fds: &[u8]) -> Vec<Marked> {
    let mut found = Vec::new();
    for file in repeated(fds, 1) {
        if string(file, 2).as_deref() != Some(PACKAGE) {
            continue;
        }
        for msg in repeated(file, 4) {
            let name = string(msg, 1).unwrap();
            assert!(
                repeated(msg, 3).is_empty(),
                "{name}: nested messages are not allowed in the contract"
            );
            let mut fields = Vec::new();
            let mut any = false;
            for field in repeated(msg, 2) {
                let fname = string(field, 1).unwrap();
                let marked = repeated(field, 8).iter().any(|opts| {
                    varint(opts, PHI) == Some(1) || varint(opts, SECRET) == Some(1)
                });
                if marked {
                    any = true;
                    assert_eq!(
                        varint(field, 5),
                        Some(TYPE_STRING),
                        "{name}.{fname}: only a string field may be marked"
                    );
                    assert!(
                        varint(field, 9).is_none(),
                        "{name}.{fname}: a marked field may not sit in a oneof"
                    );
                }
                fields.push((fname, marked, varint(field, 4) == Some(LABEL_REPEATED)));
            }
            if any {
                found.push(Marked { name, fields });
            }
        }
    }
    assert!(!found.is_empty(), "no marked field found: the walk is broken");
    found
}

fn redacted_rs(messages: &[Marked]) -> String {
    let mut s = String::from("// Generated by ehr/build.rs from the (phi) and (secret) marks. Never edit.\n\n");
    for m in messages {
        let _ = writeln!(s, "impl ::core::fmt::Debug for {} {{", m.name);
        s.push_str("    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {\n");
        let _ = writeln!(s, "        f.debug_struct({:?})", m.name);
        for (field, marked, _) in &m.fields {
            if *marked {
                let _ = writeln!(s, "            .field({field:?}, &format_args!(\"***\"))");
            } else {
                let _ = writeln!(s, "            .field({field:?}, &self.{field})");
            }
        }
        s.push_str("            .finish()\n    }\n}\n\n");
    }

    s.push_str("/// Every marked field of the contract, as (message, field).\n");
    s.push_str("pub const MARKED_FIELDS: &[(&str, &str)] = &[\n");
    for m in messages {
        for (field, marked, _) in &m.fields {
            if *marked {
                let _ = writeln!(s, "    ({:?}, {field:?}),", m.name);
            }
        }
    }
    s.push_str("];\n\n");

    s.push_str("#[cfg(test)]\nmod redacted_debug_tests {\n    use super::*;\n\n");
    s.push_str("    /// Each marked field holds a canary; no debug print may show it.\n");
    s.push_str("    #[test]\n    fn no_marked_field_reaches_a_debug_print() {\n");
    for m in messages {
        let _ = write!(s, "        let v = {} {{ ", m.name);
        for (field, marked, repeated) in &m.fields {
            if *marked && *repeated {
                let _ = write!(s, "{field}: vec![\"CANARY\".into()], ");
            } else if *marked {
                let _ = write!(s, "{field}: \"CANARY\".into(), ");
            }
        }
        s.push_str("..Default::default() };\n");
        let _ = writeln!(
            s,
            "        assert!(!format!(\"{{v:?}}\").contains(\"CANARY\"), \"{} leaks\");",
            m.name
        );
    }
    s.push_str("    }\n}\n");
    s
}

// ---- A minimal walk over protobuf wire bytes: enough for descriptors, nothing more. ----

enum Value<'a> {
    Varint(u64),
    Bytes(&'a [u8]),
    Skip,
}

fn read_varint(buf: &[u8], pos: &mut usize) -> u64 {
    let mut v = 0u64;
    let mut shift = 0;
    loop {
        let b = buf[*pos];
        *pos += 1;
        v |= u64::from(b & 0x7f) << shift;
        if b & 0x80 == 0 {
            return v;
        }
        shift += 7;
    }
}

fn fields(buf: &[u8]) -> Vec<(u64, Value<'_>)> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < buf.len() {
        let key = read_varint(buf, &mut pos);
        let value = match key & 7 {
            0 => Value::Varint(read_varint(buf, &mut pos)),
            1 => {
                pos += 8;
                Value::Skip
            }
            2 => {
                let len = read_varint(buf, &mut pos) as usize;
                let v = &buf[pos..pos + len];
                pos += len;
                Value::Bytes(v)
            }
            5 => {
                pos += 4;
                Value::Skip
            }
            w => panic!("unexpected wire type {w} in the descriptors"),
        };
        out.push((key >> 3, value));
    }
    out
}

fn repeated(buf: &[u8], number: u64) -> Vec<&[u8]> {
    fields(buf)
        .into_iter()
        .filter_map(|(n, v)| match v {
            Value::Bytes(b) if n == number => Some(b),
            _ => None,
        })
        .collect()
}

fn string(buf: &[u8], number: u64) -> Option<String> {
    repeated(buf, number)
        .first()
        .map(|b| String::from_utf8(b.to_vec()).unwrap())
}

fn varint(buf: &[u8], number: u64) -> Option<u64> {
    fields(buf).into_iter().find_map(|(n, v)| match v {
        Value::Varint(x) if n == number => Some(x),
        _ => None,
    })
}

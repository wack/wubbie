use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    // Build scripts run with the crate directory as CWD, so reach two levels up
    // (crates/<service>-client -> workspace root) for the generated spec, which
    // `cargo make generate-openapi`, CI, and `export-openapi` all write to the root.
    let openapi_path = "../../openapi.json";

    // Tell Cargo to rerun this build script if the OpenAPI spec changes
    println!("cargo:rerun-if-changed={}", openapi_path);

    // Read the OpenAPI specification
    let spec_content = fs::read_to_string(openapi_path).unwrap_or_else(|e| {
        panic!(
            "Failed to read OpenAPI spec at '{}': {}. \
             Make sure to run 'cargo make generate-openapi' first.",
            openapi_path, e
        )
    });

    // Parse as JSON Value to transform the spec
    let mut spec: serde_json::Value =
        serde_json::from_str(&spec_content).expect("Failed to parse OpenAPI spec as JSON");

    // Convert OpenAPI 3.1.0 to 3.0.3 for progenitor compatibility
    // Progenitor uses openapiv3 which only supports OpenAPI 3.0.x
    if spec
        .get("openapi")
        .and_then(|v| v.as_str())
        .is_some_and(|version| version.starts_with("3.1"))
    {
        spec["openapi"] = serde_json::json!("3.0.3");

        // Convert JSON Schema 2020-12 constructs to OpenAPI 3.0 compatible ones
        transform_schemas_recursive(&mut spec);
    }

    // Parse as OpenAPI struct
    let spec: openapiv3::OpenAPI =
        serde_json::from_value(spec).expect("Failed to parse transformed OpenAPI spec");

    // Configure the generator
    let mut settings = progenitor::GenerationSettings::new();
    settings.with_interface(progenitor::InterfaceStyle::Builder);
    settings.with_tag(progenitor::TagStyle::Merged);

    // Generate the client code
    let mut generator = progenitor::Generator::new(&settings);

    let tokens = generator
        .generate_tokens(&spec)
        .expect("Failed to generate client code from OpenAPI spec");

    // Parse and format the generated code
    let ast = syn::parse2(tokens).expect("Failed to parse generated code");
    let content = prettyplease::unparse(&ast);

    // Write to OUT_DIR
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let dest_path = out_dir.join("codegen.rs");
    fs::write(&dest_path, content).expect("Failed to write generated code");
}

/// Recursively transform schemas from OpenAPI 3.1.0/JSON Schema 2020-12 to 3.0.3 compatible format
fn transform_schemas_recursive(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            // Transform 'type' arrays to use 'nullable' (3.1.0 -> 3.0.x conversion)
            // e.g., {"type": ["string", "null"]} -> {"type": "string", "nullable": true}
            if let Some(type_val) = map.get("type").cloned()
                && let Some(types) = type_val.as_array()
            {
                let non_null_types: Vec<&serde_json::Value> = types
                    .iter()
                    .filter(|t| t.as_str() != Some("null"))
                    .collect();
                let has_null = types.iter().any(|t| t.as_str() == Some("null"));

                if non_null_types.len() == 1 {
                    map.insert("type".to_string(), non_null_types[0].clone());
                    if has_null {
                        map.insert("nullable".to_string(), serde_json::json!(true));
                    }
                }
            }

            // Convert 'const' to 'enum' with single value (3.1.0 feature)
            if let Some(const_val) = map.remove("const") {
                map.insert("enum".to_string(), serde_json::json!([const_val]));
            }

            // Convert 'examples' array to single 'example' (3.1.0 -> 3.0.x)
            if let Some(examples) = map.remove("examples")
                && let Some(arr) = examples.as_array()
                && let Some(first) = arr.first()
            {
                map.insert("example".to_string(), first.clone());
            }

            // Recursively process all nested values
            for (_, v) in map.iter_mut() {
                transform_schemas_recursive(v);
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr.iter_mut() {
                transform_schemas_recursive(item);
            }
        }
        _ => {}
    }
}

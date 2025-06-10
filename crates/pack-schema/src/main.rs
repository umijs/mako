use pack_schema::generate_schema_string;
use std::fs;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = "packages/pack/config_schema.json";

    // Generate the schema
    let schema_string = generate_schema_string()?;

    // Create output directory if it doesn't exist
    if let Some(parent) = Path::new(output_path).parent() {
        fs::create_dir_all(parent)?;
    }

    // Write the schema to file
    fs::write(output_path, schema_string)?;

    println!("✅ JSON Schema generated successfully at: {}", output_path);

    Ok(())
}

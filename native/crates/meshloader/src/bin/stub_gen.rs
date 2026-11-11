//! Generates the Python extension stub without loading the game.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stub = _meshloader::stub_info()?;
    let module = stub
        .modules
        .get_mut("meshloader._meshloader")
        .ok_or("stub module missing")?;
    module.doc =
        "Load and preview signature-discovered UE3 meshes through native Python objects.".into();
    for function in module.function.values_mut().flatten() {
        function.doc = match function.name {
            "get_reference_skeleton" => {
                "Return native-order reference bones from a loaded SkeletalMesh without mutation."
            }
            _ => {
                return Err(
                    format!("Python stub documentation missing for {}", function.name).into(),
                );
            }
        };
    }
    let output = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../meshloader/_meshloader.pyi");
    std::fs::write(&output, module.to_string())?;
    println!("wrote {}", output.display());
    Ok(())
}

use std::{fs, path::PathBuf};

const FRONTEND_CONTRACT: &str = "../src/shared/api/generated/ipc.ts";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let frontend_contract = manifest_dir.join(FRONTEND_CONTRACT);

    if let Some(parent) = frontend_contract.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(frontend_contract, postmite_lib::ipc::render_contract()?)?;

    Ok(())
}

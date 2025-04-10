use ethers_solc::{Project, ProjectPathsConfig, Solc};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Define paths
    let base_path = PathBuf::from(".");
    let contracts_dir = base_path.join("contracts");
    let contract_file = contracts_dir.join("MessageStorage.sol");
    let abi_dir = contracts_dir.clone().join("artifacts"); // Place ABI in the same directory
    let abi_file_path = abi_dir.join("MessageStorage.json"); // ABI will be written here

    // Tell Cargo to rerun this script if the contract changes
    println!("cargo:rerun-if-changed={}", contract_file.display());
    // Also rerun if the build script itself changes
    println!("cargo:rerun-if-changed=build.rs");

    // Check if solc executable is available
    if Solc::default().version().is_err() {
        eprintln!("cargo:warning=Solc compiler not found or not configured correctly. Skipping contract compilation in build script.");
        return Ok(());
    }
    //
    println!("cargo:info=Compiling contracts directory using Solc::compile_source...");
    // Define the paths to be used for the Solidity project
    let paths = ProjectPathsConfig::builder()
        .root(&contracts_dir)
        .sources(&contracts_dir)
        .artifacts(abi_dir)
        .build()?; // Build the project path configuration

    // Build the project object, enabling auto-detection of the Solidity compiler
    let project = Project::builder()
        .paths(paths)
        .set_auto_detect(true) // Automatically detect Solidity compiler
        .build()?;

    // Compile the Solidity project
    project.compile()?;
    project.rerun_if_sources_changed();
    println!("cargo:info=Compilation finished.");

    println!(
        "cargo:info=Successfully compiled contract and wrote ABI to {}",
        abi_file_path.display()
    );

    Ok(())
}

use crate::cli_config::{build_config, Command};
use ethers::abi::AbiEncode;
use ethers::contract::ContractFactory;
use ethers::middleware::SignerMiddleware;
use ethers::prelude::{LocalWallet, Signer};
use ethers::types::{BlockNumber, H256};
use ethers::utils::Anvil;
use ethers_providers::{Middleware, Provider};
use ethers_solc::{
    Artifact, ConfigurableArtifacts, Project, ProjectCompileOutput, ProjectPathsConfig,
};
use eyre::{eyre, ContextCompat, Ok, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;

mod cli_config;

const CONTRACT_FOLDER: &str = "contracts/";

#[tokio::main]
async fn main() -> Result<()> {
    let config = build_config();

    match config.command {
        Command::Deploy(config) => {
            let instance = Anvil::new()
                .mnemonic(config.mnemonic)
                .block_time(1u64)
                .spawn();

            println!("HTTP Endpoint: {}", instance.endpoint()); // Print the Ganache instance's HTTP endpoint
            let wallet: LocalWallet = instance.keys()[0].clone().into();
            let first_address = wallet.address(); // Get the wallet's address (derived from the private key)
            println!(
                "wallet first address: {}",
                first_address.encode_hex() // Convert the address to hexadecimal and print it
            );
            let provider =
                Provider::try_from(instance.endpoint())?.interval(Duration::from_millis(10)); // Set polling interval
            let chain_id = provider.get_chainid().await?; // Get the chain ID for the Ethereum network
            println!("Ganache started with chain id {}", chain_id); // Print the chain ID

            let project = compile(CONTRACT_FOLDER).await?;
            print_project(project.clone()).await?;
            let balance = provider.get_balance(wallet.address(), None).await?;
            println!(
                "Wallet first address {} balance: {}",
                wallet.address().encode_hex(), // Encode the address to hexadecimal for printing
                balance
            );

            let contract_name = config.contract_name;
            let contract_absolute_str = std::fs::canonicalize(
                Path::new(CONTRACT_FOLDER).join(contract_name.clone() + ".sol"),
            )?;
            let contract_absolute_str = contract_absolute_str.to_str().unwrap();

            println!("contract path: {}", contract_absolute_str);
            let contract = project
                .find(contract_absolute_str, contract_name) // Find the contract by its name and path
                .context("Contract not found")? // Handle the case where the contract is not found
                .clone(); // Clone the contract (ownership handling)

            let (abi, bytecode, _) = contract.into_parts();
            let abi = abi.context("Missing abi from contract")?; // Ensure that ABI is available
            let bytecode = bytecode.context("Missing bytecode from contract")?; // Ensure that bytecode is available
            let wallet = wallet.with_chain_id(chain_id.as_u64());
            let client = SignerMiddleware::new(provider.clone(), wallet).into();
            let factory = ContractFactory::new(abi.clone(), bytecode, client);

            let deployer = factory.deploy(())?;
            let block = provider
                .clone()
                .get_block(BlockNumber::Latest)
                .await?
                .context("Failed to get block");
            let block = block?;
            println!("Block num: {:?}", block.clone().number);

            let gas_price = block
                .next_block_base_fee()
                .context("Failed to get the base fee for the next block")?;
            // deployer.tx.set_gas_price::<U256>(gas_price+1000); // Set gas price for the transaction

            println!("block gas price: {}", gas_price);

            let contract = deployer.clone().legacy().send().await?;
            println!(
                "Contract address: {}",
                contract.address().encode_hex() // Print the deployed contract's address
            );

            let call =
                contract.method::<_, H256>("writeMessage", "1 Hello Solidity!".to_owned())?;

            let pending_tx = call.send().await?;
            let receipt = pending_tx.confirmations(1).await?;
            println!("gas used: {:?}", receipt.unwrap().gas_used);

            let call =
                contract.method::<_, H256>("writeMessage", "2 Hello Solidity!".to_owned())?;
            let pending_tx = call.send().await?;
            let receipt = pending_tx.confirmations(1).await?;
            println!("gas used: {:?}", receipt.unwrap().gas_used);

            let messages: Vec<String> = contract.method("getMessages", ())?.call().await?;
            println!("messages: {:?}", messages);
        }
    }

    Ok(())
}

// Function to compile a Solidity project from the given root folder path
pub async fn compile(root: &str) -> Result<ProjectCompileOutput<ConfigurableArtifacts>> {
    let root = PathBuf::from(root); // Convert the root folder path to a PathBuf object
    if !root.exists() {
        return Err(eyre!("Project root {root:?} does not exist!")); // Error handling for non-existent project root
    }

    // Define the paths to be used for the Solidity project
    let paths = ProjectPathsConfig::builder()
        .root(&root)
        .sources(&root)
        .build()?; // Build the project path configuration

    // Build the project object, enabling auto-detection of the Solidity compiler
    let project = Project::builder()
        .paths(paths)
        .set_auto_detect(true) // Automatically detect Solidity compiler
        .no_artifacts() // Avoid writing artifacts to disk
        .build()?;

    // Compile the Solidity project
    let output = project.compile()?;

    // Check if there were any compiler errors
    if output.has_compiler_errors() {
        Err(eyre!(
            "Compiling solidity project failed: {:?}",
            output.output().errors // Print compilation errors
        ))
    } else {
        Ok(output.clone()) // Return the compiled output if successful
    }
}

pub async fn print_project(project: ProjectCompileOutput<ConfigurableArtifacts>) -> Result<()> {
    let artifacts = project.into_artifacts(); // Extract the compiled artifacts (contracts)
    for (id, artifact) in artifacts {
        let name = id.name; // Get the contract's name
        let abi = artifact.abi.context("No ABI found for artifact {name}")?; // Get the ABI and ensure it exists

        println!("{}", "=".repeat(80)); // Print a separator
        println!("CONTRACT: {:?}", name); // Print the contract name

        let contract = &abi.abi;
        let functions = contract.functions(); // Get the list of functions from the contract
        let functions = functions.cloned(); // Clone the function list for iteration
        let constructor = contract.constructor(); // Get the constructor if available

        // If the contract has a constructor, print its arguments
        if let Some(constructor) = constructor {
            let args = &constructor.inputs;
            println!("CONSTRUCTOR args: {:?}", args); // Print the constructor arguments
        }

        // Print each function's name and parameters
        for func in functions {
            let name = &func.name; // Get the function name
            let params = &func.inputs; // Get the function parameters
            println!("FUNCTION {name} {params:?}"); // Print function details
        }
    }
    Ok(())
}

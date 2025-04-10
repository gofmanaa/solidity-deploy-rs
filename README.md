# solidity-deploy-rs


This project appears to be a Rust application for deploying and interacting with Solidity smart contracts on Ethereum-like blockchains. It includes tools for deployment and potentially a web server interface.

## Project Structure

-   `contracts/`: Contains the Solidity smart contracts (e.g., `MessageStorage.sol`).
-   `src/`: Contains the source code for the `deploy` binary.
-   `server/`: Contains the source code for the `server` binary.
-   `Cargo.toml`: Project manifest and dependencies.
-   `build.rs`: Build script, likely for compiling Solidity contracts.

## Binaries

This project provides two main binaries:

1.  **`deploy`**: Located at `src/main.rs`. Used for deploying smart contracts to a blockchain.
2.  **`server`**: Located at `server/main.rs`. Runs a web server (using Actix-web) for interacting with the deployed contracts or managing deployments.

## Building and Running

1.  **Build the project:**
    ```bash
    cargo build --release
    ```

2.  **Run the deployer:**
    *(Specific command-line arguments might be required. Check `src/cli_config.rs` or run with `--help`)*
    ```bash
    ./target/release/deploy --help 
    ```

3.  **Run the server:**
    *(Specific configuration or environment variables might be needed)*
    ```bash
    ./target/release/server
    ```

## Smart Contracts

The primary smart contract seems to be `MessageStorage.sol`. The build process likely compiles this contract using `ethers-solc`.

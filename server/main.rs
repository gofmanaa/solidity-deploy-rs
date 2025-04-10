use actix_web::{
    middleware::Logger, web, App, HttpResponse, HttpServer, Responder, Result as ActixResult,
};
use ethers::prelude::k256::ecdsa::SigningKey;
use ethers::{
    prelude::*,
    utils::{Anvil, AnvilInstance},
};
use ethers_solc::artifacts::CompactContractBytecode;
use eyre::Result;
use serde::{Deserialize, Serialize};
use std::{fs::File, io::BufReader, path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::Mutex;

// Define a struct for the request body of /api/store-message
#[derive(Deserialize, Debug)]
struct StoreMessageRequest {
    message: String,
}

// Define a struct for the response body of /api/retrieve-messages
#[derive(Serialize)]
struct RetrieveMessagesResponse {
    messages: Vec<String>,
}

// Define the contract binding - expects ABI at compile time
abigen!(
    MessageStorage,
    "contracts/artifacts/MessageStorage.sol/MessageStorage.json",
    event_derives(serde::Deserialize, serde::Serialize)
);

// Event struct generated from `abigen!`
#[derive(Debug, Clone, Default, EthEvent)]
pub struct MessageWritten {
    pub message: String,
    #[ethevent(indexed)]
    pub sender: Address,
}

impl MessageStorage<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>> {
    pub async fn subscribe_to_events(self) -> Result<()> {
        tokio::spawn(async move {
            // Create a stream to listen for events
            let ev = self.event::<MessageWritten>().from_block(0);
            let mut event_stream = match ev.stream().await {
                Ok(stream) => stream.take(1),
                Err(e) => {
                    log::error!("Failed to start event stream: {}", e);
                    return;
                }
            };
            // Subscribe to events and handle them
            while let Some(Ok(event)) = event_stream.next().await {
                log::info!(
                    "📨 New message: {}, 🧑 Sender: {:?}",
                    event.message,
                    event.sender
                );
            }
        });

        Ok(())
    }
}

// Shared application state
struct AppState {
    contract: MessageStorage<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
}

// Handler for the root endpoint "/"
async fn index() -> impl Responder {
    HttpResponse::Ok().body("Welcome to the Message Storage Server!")
}

// Handler for POST /api/store-message
async fn store_message_handler(
    req: web::Json<StoreMessageRequest>,
    data: web::Data<Arc<Mutex<AppState>>>,
) -> ActixResult<impl Responder> {
    let app_state = data.lock().await;
    let contract = &app_state.contract;
    let message_to_store = req.into_inner().message;

    log::info!("Received request to store message: {}", message_to_store);

    let message_clone = message_to_store.clone();

    match contract.write_message(message_to_store).send().await {
        Ok(pending_tx) => {
            log::info!(
                "Transaction sent for message '{}', waiting for confirmation...",
                message_clone
            );
            // Wait for confirmation with a timeout
            match pending_tx.interval(Duration::from_millis(100)).await {
                Ok(Some(receipt)) => {
                    log::info!(
                        "Message '{}' stored successfully! Transaction hash: {:?}",
                        message_clone,
                        receipt.transaction_hash
                    );
                    Ok(HttpResponse::Ok().json(serde_json::json!({ "status": "success", "tx_hash": receipt.transaction_hash })))
                }
                Ok(None) => {
                    log::error!(
                        "Transaction for message '{}' dropped from mempool",
                        message_clone
                    );
                    Ok(HttpResponse::InternalServerError().json(
                        serde_json::json!({ "status": "error", "message": "Transaction dropped" }),
                    ))
                }
                Err(e) => {
                    log::error!(
                        "Error waiting for transaction confirmation for message '{}': {}",
                        message_clone,
                        e
                    );
                    Ok(HttpResponse::InternalServerError().json(serde_json::json!({ "status": "error", "message": format!("Transaction confirmation failed: {}", e) })))
                }
            }
        }
        Err(e) => {
            log::error!(
                "Failed to send write_message transaction for message '{}': {}",
                message_clone,
                e
            );
            // Check for common contract errors (like revert)
            if let Some(contract_error) = e.as_revert() {
                log::error!("Contract reverted: {:?}", contract_error);
                Ok(HttpResponse::BadRequest().json(serde_json::json!({ "status": "error", "message": format!("Contract execution failed: {:?}", contract_error) })))
            } else {
                Ok(HttpResponse::InternalServerError().json(serde_json::json!({ "status": "error", "message": format!("Failed to send transaction: {:?}", e) })))
            }
        }
    }
}

// Handler for GET /api/retrieve-messages
async fn retrieve_messages_handler(
    data: web::Data<Arc<Mutex<AppState>>>,
) -> ActixResult<impl Responder> {
    let app_state = data.lock().await;
    let contract = &app_state.contract;

    log::info!("Received request to retrieve messages");

    match contract.get_messages().call().await {
        Ok(messages) => {
            log::info!("Retrieved {} messages", messages.len());
            Ok(HttpResponse::Ok().json(RetrieveMessagesResponse { messages }))
        }
        Err(e) => {
            log::error!("Failed to call get_messages: {}", e);
            if let Some(contract_error) = e.as_revert() {
                log::error!(
                    "Contract reverted during get_messages: {:?}",
                    contract_error
                );
                Ok(HttpResponse::InternalServerError().json(serde_json::json!({ "status": "error", "message": format!("Contract execution failed during retrieval: {:?}", contract_error) })))
            } else {
                Ok(HttpResponse::InternalServerError().json(serde_json::json!({ "status": "error", "message": format!("Failed to retrieve messages: {}", e) })))
            }
        }
    }
}

// Function to compile and deploy the contract
async fn setup_contract() -> Result<(
    MessageStorage<SignerMiddleware<Provider<Http>, Wallet<k256::ecdsa::SigningKey>>>,
    AnvilInstance,
)> {
    let anvil = Anvil::default().spawn();
    log::info!("Anvil started at endpoint: {}", anvil.endpoint());
    //log::info!("Anvil addresses: {:?}", anvil.addresses());
    // Get the first default Anvil account
    let wallet: LocalWallet = anvil.keys()[0].clone().into();

    let provider =
        Provider::<Http>::try_from(anvil.endpoint())?.interval(Duration::from_millis(10u64));

    let balance = provider.clone().get_balance(wallet.address(), None).await?;
    log::info!("Address: {}, balance: {}", wallet.address(), balance);

    log::info!("Loading contract ABI and bytecode from file...");
    let abi_path = PathBuf::from("./contracts/artifacts/MessageStorage.sol/MessageStorage.json");
    let file = File::open(&abi_path)
        .map_err(|e| eyre::eyre!("Failed to open ABI file {:?}: {}", abi_path, e))?;
    if abi_path.exists() {
        println!("{}", abi_path.display())
    }
    let reader = BufReader::new(file);
    let contract_artifact: CompactContractBytecode = serde_json::from_reader(reader)
        .map_err(|e| eyre::eyre!("Failed to parse ABI file {:?}: {}", abi_path, e))?;

    let abi = contract_artifact
        .abi
        .clone()
        .ok_or_else(|| eyre::eyre!("ABI not found in artifact file"))?;

    let bytecode: Bytes = contract_artifact
        .bytecode
        .clone()
        .ok_or_else(|| eyre::eyre!("Bytecode not found in artifact file"))?
        .object
        .into_bytes()
        .ok_or_else(|| eyre::eyre!("Bytecode object is not valid bytes"))?;

    let client = Arc::new(SignerMiddleware::new(
        provider,
        wallet.with_chain_id(anvil.chain_id()),
    ));

    log::info!("Deploying contract...");
    let factory = ContractFactory::new(abi.clone(), bytecode, client.clone());
    let deployer = factory
        .deploy(())? // constructor arguments hire
        .legacy();

    let contract_instance = deployer.send().await?;

    let contract_address = contract_instance.address();
    log::info!("Contract deployed at address: {:?}", contract_address);

    let contract = MessageStorage::new(contract_address, client.clone());

    // Subscribe to events
    contract.clone().subscribe_to_events().await?;

    Ok((contract, anvil))
}

#[actix_web::main]
async fn main() -> Result<()> {
    // Use RUST_LOG=info cargo run --bin server
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let (contract_instance, _anvil_instance) =
        setup_contract().await.expect("Failed to setup contract");

    // Create shared state
    let app_state = Arc::new(Mutex::new(AppState {
        contract: contract_instance,
    }));

    // Start Actix-web server
    let server_address = "127.0.0.1";
    let server_port = 8080;
    log::info!(
        "Starting HTTP server at http://{}:{}",
        server_address,
        server_port
    );

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .app_data(web::Data::new(app_state.clone()))
            .route("/", web::get().to(index))
            .route("/api/store-message", web::post().to(store_message_handler))
            .route(
                "/api/retrieve-messages",
                web::get().to(retrieve_messages_handler),
            )
    })
    .bind((server_address, server_port))?
    .run()
    .await?;

    Ok(())
}

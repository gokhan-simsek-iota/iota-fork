// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{env, net::SocketAddr, path::PathBuf};

use diesel::{connection::SimpleConnection, r2d2::R2D2Connection};
use iota_json_rpc_types::IotaTransactionBlockResponse;
use iota_metrics::init_metrics;
use secrecy::{ExposeSecret, Secret};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::{
    IndexerConfig, IndexerMetrics,
    db::{ConnectionPoolConfig, new_connection_pool_with_config},
    errors::IndexerError,
    handlers::objects_snapshot_processor::SnapshotLagConfig,
    indexer::Indexer,
    store::PgIndexerStore,
};

pub enum ReaderWriterConfig {
    Reader { reader_mode_rpc_url: String },
    Writer { snapshot_config: SnapshotLagConfig },
}

impl ReaderWriterConfig {
    pub fn reader_mode(reader_mode_rpc_url: String) -> Self {
        Self::Reader {
            reader_mode_rpc_url,
        }
    }

    pub fn writer_mode(snapshot_config: Option<SnapshotLagConfig>) -> Self {
        Self::Writer {
            snapshot_config: snapshot_config.unwrap_or_default(),
        }
    }
}

pub async fn start_test_indexer<T: R2D2Connection + Send + 'static>(
    db_url: Option<String>,
    rpc_url: String,
    reader_writer_config: ReaderWriterConfig,
    data_ingestion_path: Option<PathBuf>,
    new_database: Option<&str>,
) -> (PgIndexerStore<T>, JoinHandle<Result<(), IndexerError>>) {
    start_test_indexer_impl(
        db_url,
        rpc_url,
        reader_writer_config,
        // reset_database
        false,
        data_ingestion_path,
        CancellationToken::new(),
        new_database,
    )
    .await
}

/// Starts an indexer reader or writer for testing depending on the
/// `reader_writer_config`. If `reset_database` is true, the database instance
/// named in `db_url` will be dropped and reinstantiated.
pub async fn start_test_indexer_impl<T: R2D2Connection + 'static>(
    db_url: Option<String>,
    rpc_url: String,
    reader_writer_config: ReaderWriterConfig,
    mut reset_database: bool,
    data_ingestion_path: Option<PathBuf>,
    cancel: CancellationToken,
    new_database: Option<&str>,
) -> (PgIndexerStore<T>, JoinHandle<Result<(), IndexerError>>) {
    let mut db_url = db_url.unwrap_or_else(|| {
        let pg_host = env::var("POSTGRES_HOST").unwrap_or_else(|_| "localhost".into());
        let pg_port = env::var("POSTGRES_PORT").unwrap_or_else(|_| "32770".into());
        let pw = env::var("POSTGRES_PASSWORD").unwrap_or_else(|_| "postgrespw".into());
        format!("postgres://postgres:{pw}@{pg_host}:{pg_port}")
    });

    if let Some(new_database) = new_database {
        db_url = replace_db_name(&db_url, new_database).0;
        reset_database = true;
    };

    let mut config = IndexerConfig {
        db_url: Some(db_url.clone().into()),
        // As fallback sync mechanism enable Rest Api if `data_ingestion_path` was not provided
        remote_store_url: data_ingestion_path
            .is_none()
            .then_some(format!("{rpc_url}/api/v1")),
        rpc_client_url: rpc_url,
        reset_db: true,
        fullnode_sync_worker: true,
        rpc_server_worker: false,
        data_ingestion_path,
        ..Default::default()
    };

    let store = create_pg_store(config.get_db_url().unwrap(), reset_database);

    let registry = prometheus::Registry::default();
    let handle = match reader_writer_config {
        ReaderWriterConfig::Reader {
            reader_mode_rpc_url,
        } => {
            let reader_mode_rpc_url = reader_mode_rpc_url
                .parse::<SocketAddr>()
                .expect("Unable to parse fullnode address");
            config.fullnode_sync_worker = false;
            config.rpc_server_worker = true;
            config.rpc_server_url = reader_mode_rpc_url.ip().to_string();
            config.rpc_server_port = reader_mode_rpc_url.port();
            tokio::spawn(
                async move { Indexer::start_reader::<T>(&config, &registry, db_url).await },
            )
        }
        ReaderWriterConfig::Writer { snapshot_config } => {
            if config.reset_db {
                let blocking_pool =
                    new_connection_pool_with_config::<T>(&db_url, Some(5), Default::default())
                        .unwrap();
                crate::db::reset_database(&mut blocking_pool.get().unwrap()).unwrap();
            }

            let store_clone = store.clone();

            init_metrics(&registry);
            let indexer_metrics = IndexerMetrics::new(&registry);

            tokio::spawn(async move {
                Indexer::start_writer_with_config::<PgIndexerStore<T>, T>(
                    &config,
                    store_clone,
                    indexer_metrics,
                    snapshot_config,
                    cancel,
                )
                .await
            })
        }
    };

    (store, handle)
}

pub fn create_pg_store<T: R2D2Connection + Send + 'static>(
    db_url: Secret<String>,
    reset_database: bool,
) -> PgIndexerStore<T> {
    // Reduce the connection pool size to 10 for testing
    // to prevent maxing out
    info!("Setting DB_POOL_SIZE to 10");
    std::env::set_var("DB_POOL_SIZE", "10");

    // Set connection timeout for tests to 1 second
    let pool_config = ConnectionPoolConfig::default();

    let registry = prometheus::Registry::default();

    init_metrics(&registry);

    let indexer_metrics = IndexerMetrics::new(&registry);

    let mut parsed_url = db_url.clone();
    if reset_database {
        let db_name = parsed_url.expose_secret().split('/').last().unwrap();
        // Switch to default to create a new database
        let (default_db_url, _) = replace_db_name(parsed_url.expose_secret(), "postgres");

        // Open in default mode
        let blocking_pool =
            new_connection_pool_with_config::<T>(&default_db_url, Some(5), pool_config).unwrap();
        let mut default_conn = blocking_pool.get().unwrap();

        // Delete the old db if it exists
        default_conn
            .batch_execute(&format!("DROP DATABASE IF EXISTS {}", db_name))
            .unwrap();

        // Create the new db
        default_conn
            .batch_execute(&format!("CREATE DATABASE {}", db_name))
            .unwrap();
        parsed_url = replace_db_name(parsed_url.expose_secret(), db_name)
            .0
            .into();
    }

    let blocking_pool =
        new_connection_pool_with_config::<T>(parsed_url.expose_secret(), Some(5), pool_config)
            .unwrap();
    PgIndexerStore::new(blocking_pool.clone(), indexer_metrics.clone())
}

fn replace_db_name(db_url: &str, new_db_name: &str) -> (String, String) {
    let pos = db_url.rfind('/').expect("Unable to find / in db_url");
    let old_db_name = &db_url[pos + 1..];

    (
        format!("{}/{}", &db_url[..pos], new_db_name),
        old_db_name.to_string(),
    )
}

pub async fn force_delete_database<T: R2D2Connection + 'static>(db_url: String) {
    // Replace the database name with the default `postgres`, which should be the
    // last string after `/` This is necessary because you can't drop a database
    // while being connected to it. Hence switch to the default `postgres`
    // database to drop the active database.
    let (default_db_url, db_name) = replace_db_name(&db_url, "postgres");
    let pool_config = ConnectionPoolConfig::default();

    let blocking_pool =
        new_connection_pool_with_config::<T>(&default_db_url, Some(5), pool_config).unwrap();
    blocking_pool
        .get()
        .unwrap()
        .batch_execute(&format!("DROP DATABASE IF EXISTS {} WITH (FORCE)", db_name))
        .unwrap();
}

#[derive(Clone)]
pub struct IotaTransactionBlockResponseBuilder<'a> {
    response: IotaTransactionBlockResponse,
    full_response: &'a IotaTransactionBlockResponse,
}

impl<'a> IotaTransactionBlockResponseBuilder<'a> {
    pub fn new(full_response: &'a IotaTransactionBlockResponse) -> Self {
        Self {
            response: IotaTransactionBlockResponse::default(),
            full_response,
        }
    }

    pub fn with_input(mut self) -> Self {
        self.response = IotaTransactionBlockResponse {
            transaction: self.full_response.transaction.clone(),
            ..self.response
        };
        self
    }

    pub fn with_raw_input(mut self) -> Self {
        self.response = IotaTransactionBlockResponse {
            raw_transaction: self.full_response.raw_transaction.clone(),
            ..self.response
        };
        self
    }

    pub fn with_effects(mut self) -> Self {
        self.response = IotaTransactionBlockResponse {
            effects: self.full_response.effects.clone(),
            ..self.response
        };
        self
    }

    pub fn with_events(mut self) -> Self {
        self.response = IotaTransactionBlockResponse {
            events: self.full_response.events.clone(),
            ..self.response
        };
        self
    }

    pub fn with_balance_changes(mut self) -> Self {
        self.response = IotaTransactionBlockResponse {
            balance_changes: self.full_response.balance_changes.clone(),
            ..self.response
        };
        self
    }

    pub fn with_object_changes(mut self) -> Self {
        self.response = IotaTransactionBlockResponse {
            object_changes: self.full_response.object_changes.clone(),
            ..self.response
        };
        self
    }

    pub fn with_input_and_changes(mut self) -> Self {
        self.response = IotaTransactionBlockResponse {
            transaction: self.full_response.transaction.clone(),
            balance_changes: self.full_response.balance_changes.clone(),
            object_changes: self.full_response.object_changes.clone(),
            ..self.response
        };
        self
    }

    pub fn build(self) -> IotaTransactionBlockResponse {
        IotaTransactionBlockResponse {
            transaction: self.response.transaction,
            raw_transaction: self.response.raw_transaction,
            effects: self.response.effects,
            events: self.response.events,
            balance_changes: self.response.balance_changes,
            object_changes: self.response.object_changes,
            // Use full response for any fields that aren't showable
            ..self.full_response.clone()
        }
    }
}

// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{any::Any, collections::BTreeMap};

use async_trait::async_trait;

use crate::{
    errors::IndexerError,
    handlers::{EpochToCommit, TransactionObjectChangesToCommit},
    models::{
        display::StoredDisplay,
        objects::{StoredDeletedObject, StoredObject},
    },
    types::{
        EventIndex, IndexedCheckpoint, IndexedEvent, IndexedPackage, IndexedTransaction, TxIndex,
    },
};

#[expect(clippy::large_enum_variant)]
pub enum ObjectChangeToCommit {
    MutatedObject(StoredObject),
    DeletedObject(StoredDeletedObject),
}

#[async_trait]
pub trait IndexerStore: Any + Clone + Sync + Send + 'static {
    async fn get_latest_checkpoint_sequence_number(&self) -> Result<Option<u64>, IndexerError>;

    async fn get_available_epoch_range(&self) -> Result<(u64, u64), IndexerError>;

    async fn get_available_checkpoint_range(&self) -> Result<(u64, u64), IndexerError>;

    async fn get_latest_object_snapshot_checkpoint_sequence_number(
        &self,
    ) -> Result<Option<u64>, IndexerError>;

    async fn get_chain_identifier(&self) -> Result<Option<Vec<u8>>, IndexerError>;

    fn persist_protocol_configs_and_feature_flags(
        &self,
        chain_id: Vec<u8>,
    ) -> Result<(), IndexerError>;

    async fn persist_objects(
        &self,
        object_changes: Vec<TransactionObjectChangesToCommit>,
    ) -> Result<(), IndexerError>;

    async fn persist_object_history(
        &self,
        object_changes: Vec<TransactionObjectChangesToCommit>,
    ) -> Result<(), IndexerError>;

    // persist objects snapshot with object changes during backfill
    async fn backfill_objects_snapshot(
        &self,
        object_changes: Vec<TransactionObjectChangesToCommit>,
    ) -> Result<(), IndexerError>;

    // update objects snapshot after backfill is done
    async fn update_objects_snapshot(&self, start_cp: u64, end_cp: u64)
    -> Result<(), IndexerError>;

    async fn persist_checkpoints(
        &self,
        checkpoints: Vec<IndexedCheckpoint>,
    ) -> Result<(), IndexerError>;

    async fn persist_transactions(
        &self,
        transactions: Vec<IndexedTransaction>,
    ) -> Result<(), IndexerError>;

    async fn persist_tx_indices(&self, indices: Vec<TxIndex>) -> Result<(), IndexerError>;

    async fn persist_events(&self, events: Vec<IndexedEvent>) -> Result<(), IndexerError>;
    async fn persist_event_indices(
        &self,
        event_indices: Vec<EventIndex>,
    ) -> Result<(), IndexerError>;

    async fn persist_displays(
        &self,
        display_updates: BTreeMap<String, StoredDisplay>,
    ) -> Result<(), IndexerError>;

    async fn persist_packages(&self, packages: Vec<IndexedPackage>) -> Result<(), IndexerError>;

    async fn persist_epoch(&self, epoch: EpochToCommit) -> Result<(), IndexerError>;

    async fn advance_epoch(&self, epoch: EpochToCommit) -> Result<(), IndexerError>;

    async fn prune_epoch(&self, epoch: u64) -> Result<(), IndexerError>;

    async fn get_network_total_transactions_by_end_of_epoch(
        &self,
        epoch: u64,
    ) -> Result<u64, IndexerError>;

    fn as_any(&self) -> &dyn Any;
}

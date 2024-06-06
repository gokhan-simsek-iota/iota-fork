// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type EndOfEpochInfo } from '@iota/iota.js/client';

export function getEpochStorageFundFlow(endOfEpochInfo: EndOfEpochInfo | null) {
    const fundInflow = endOfEpochInfo
        ? BigInt(endOfEpochInfo.storageFundReinvestment) +
          BigInt(endOfEpochInfo.storageCharge) +
          BigInt(endOfEpochInfo.leftoverStorageFundInflow)
        : null;

    const fundOutflow = endOfEpochInfo ? BigInt(endOfEpochInfo.storageRebate) : null;

    const netInflow = fundInflow !== null && fundOutflow !== null ? fundInflow - fundOutflow : null;

    return { netInflow, fundInflow, fundOutflow };
}

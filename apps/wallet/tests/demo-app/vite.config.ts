// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';
import tsconfigPaths from 'vite-tsconfig-paths';

export default defineConfig({
    plugins: [react(), tsconfigPaths({ root: '../../' })],
    resolve: {
        alias: {
            '@iota/bcs': new URL('../../../../sdk/bcs/src', import.meta.url).pathname,
        },
    },
});

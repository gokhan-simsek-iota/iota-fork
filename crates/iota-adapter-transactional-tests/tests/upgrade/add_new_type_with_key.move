// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# init --addresses Test_V0=0x0 Test_V1=0x0 --accounts A

//# publish --upgradeable --sender A
module Test_V0::base {
    public struct Foo {
        id: UID,
    }
}

//# upgrade --package Test_V0 --upgrade-capability 1,1 --sender A
module Test_V1::base {
    public struct Foo {
        id: UID,
    }
    public struct Bar has key {
        id: UID,
    }
}

// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Example demonstrating building and compiling two native token packages.

use iota_sdk::Url;
use sui_genesis_builder::stardust::native_token::package_builder::PackageBuilder;
use sui_genesis_builder::stardust::native_token::package_data::{
    MoveTomlManifest, NativeTokenModuleData, NativeTokenPackageData,
};

fn main() -> anyhow::Result<()> {
    let package_builder = PackageBuilder;

    let native_token_a = NativeTokenPackageData::new(
        MoveTomlManifest::new("native_token_example".to_string()),
        NativeTokenModuleData::new(
            "324823948".to_string(),
            "doge".to_string(),
            "DOGE".to_string(),
            0,
            "DOGE".to_string(),
            100_000_000_000,
            "Dogecoin".to_string(),
            "Much wow".to_string(),
            Some(Url::parse("https://raw.githubusercontent.com/dogecoin/dogecoin/master/share/pixmaps/dogecoin256.png").unwrap()),
            "0x54654".to_string(),
        ),
    );

    let compiled_package_a = package_builder.build_and_compile(native_token_a)?;
    println!("Compiled package: {:?}", compiled_package_a);

    let native_token_b = NativeTokenPackageData::new(
        MoveTomlManifest::new("native_token_example".to_string()),
        NativeTokenModuleData::new(
            "34543525".to_string(),
            "smr".to_string(),
            "SMR".to_string(),
            0,
            "SMR".to_string(),
            10_000_000_000,
            "Shimmer".to_string(),
            "Shimmy Shimmy Ya".to_string(),
            Option::None,
            "0x54654".to_string(),
        ),
    );

    let compiled_package_b = package_builder.build_and_compile(native_token_b)?;
    println!("Compiled package: {:?}", compiled_package_b);

    Ok(())
}

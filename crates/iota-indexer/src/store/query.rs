// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_types::IotaObjectDataFilter;
use iota_types::base_types::ObjectID;

pub trait DBFilter<C> {
    fn to_objects_history_sql(&self, cursor: Option<C>, limit: usize, columns: Vec<&str>)
    -> String;
    fn to_latest_objects_sql(&self, cursor: Option<C>, limit: usize, columns: Vec<&str>) -> String;
}

impl DBFilter<ObjectID> for IotaObjectDataFilter {
    fn to_objects_history_sql(
        &self,
        cursor: Option<ObjectID>,
        limit: usize,
        columns: Vec<&str>,
    ) -> String {
        let inner_clauses = to_clauses(self);
        let inner_clauses = if let Some(inner_clauses) = inner_clauses {
            format!("\n      AND {inner_clauses}")
        } else {
            "".to_string()
        };
        let outer_clauses = to_outer_clauses(self);
        let outer_clauses = if let Some(outer_clauses) = outer_clauses {
            format!("\nAND {outer_clauses}")
        } else {
            "".to_string()
        };
        let cursor = if let Some(cursor) = cursor {
            format!("\n      AND o.object_id > '{cursor}'")
        } else {
            "".to_string()
        };

        let columns = columns
            .iter()
            .map(|c| format!("t1.{c}"))
            .collect::<Vec<_>>()
            .join(", ");
        // NOTE: order by checkpoint DESC so that whenever a row from checkpoint is
        // available, we will pick that over the one from fast-path, which has
        // checkpoint of -1.
        format!(
            "SELECT {columns}
FROM (SELECT DISTINCT ON (o.object_id) *
      FROM objects_history o
      WHERE o.checkpoint <= $1{cursor}{inner_clauses}
      ORDER BY o.object_id, version, o.checkpoint DESC) AS t1
WHERE t1.object_status NOT IN ('deleted', 'wrapped', 'unwrapped_then_deleted'){outer_clauses}
LIMIT {limit};"
        )
    }

    fn to_latest_objects_sql(
        &self,
        cursor: Option<ObjectID>,
        limit: usize,
        columns: Vec<&str>,
    ) -> String {
        let columns = columns
            .iter()
            .map(|c| format!("o.{c}"))
            .collect::<Vec<_>>()
            .join(", ");

        let cursor = if let Some(cursor) = cursor {
            format!(" AND o.object_id > '{cursor}'")
        } else {
            "".to_string()
        };

        let inner_clauses = to_latest_objects_clauses(self);
        let inner_clauses = if let Some(inner_clauses) = inner_clauses {
            format!(" AND {inner_clauses}")
        } else {
            "".to_string()
        };

        format!(
            "SELECT {columns}
FROM objects o WHERE o.object_status NOT IN ('deleted', 'wrapped', 'unwrapped_then_deleted'){cursor}{inner_clauses}
LIMIT {limit};"
        )
    }
}

fn to_latest_objects_clauses(filter: &IotaObjectDataFilter) -> Option<String> {
    match filter {
        IotaObjectDataFilter::AddressOwner(a) => Some(format!(
            "(o.owner_type = 'address_owner' AND o.owner_address = '{a}')"
        )),
        _ => None,
    }
}

fn to_clauses(filter: &IotaObjectDataFilter) -> Option<String> {
    match filter {
        IotaObjectDataFilter::MatchAll(sub_filters) => {
            let sub_filters = sub_filters.iter().flat_map(to_clauses).collect::<Vec<_>>();
            if sub_filters.is_empty() {
                None
            } else if sub_filters.len() == 1 {
                Some(sub_filters[0].to_string())
            } else {
                Some(format!("({})", sub_filters.join(" AND ")))
            }
        }
        IotaObjectDataFilter::MatchAny(sub_filters) => {
            let sub_filters = sub_filters.iter().flat_map(to_clauses).collect::<Vec<_>>();
            if sub_filters.is_empty() {
                // Any default to false
                Some("FALSE".to_string())
            } else if sub_filters.len() == 1 {
                Some(sub_filters[0].to_string())
            } else {
                Some(format!("({})", sub_filters.join(" OR ")))
            }
        }
        IotaObjectDataFilter::MatchNone(sub_filters) => {
            let sub_filters = sub_filters.iter().flat_map(to_clauses).collect::<Vec<_>>();
            if sub_filters.is_empty() {
                None
            } else {
                Some(format!("NOT ({})", sub_filters.join(" OR ")))
            }
        }
        IotaObjectDataFilter::Package(p) => {
            Some(format!("o.object_type LIKE '{}::%'", p.to_hex_literal()))
        }
        IotaObjectDataFilter::MoveModule { package, module } => Some(format!(
            "o.object_type LIKE '{}::{}::%'",
            package.to_hex_literal(),
            module
        )),
        IotaObjectDataFilter::StructType(s) => {
            // If people do not provide type_params, we will match all type_params
            // e.g. `0x2::coin::Coin` can match `0x2::coin::Coin<0x2::iota::IOTA>`
            if s.type_params.is_empty() {
                Some(format!("o.object_type LIKE '{s}%'"))
            } else {
                Some(format!("o.object_type = '{s}'"))
            }
        }
        IotaObjectDataFilter::AddressOwner(a) => Some(format!(
            "((o.owner_type = 'address_owner' AND o.owner_address = '{a}') OR (o.old_owner_type = 'address_owner' AND o.old_owner_address = '{a}'))"
        )),
        IotaObjectDataFilter::ObjectOwner(o) => Some(format!(
            "((o.owner_type = 'object_owner' AND o.owner_address = '{o}') OR (o.old_owner_type = 'object_owner' AND o.old_owner_address = '{o}'))"
        )),
        IotaObjectDataFilter::ObjectId(id) => Some(format!("o.object_id = '{id}'")),
        IotaObjectDataFilter::ObjectIds(ids) => {
            if ids.is_empty() {
                None
            } else {
                let ids = ids
                    .iter()
                    .map(|o| o.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                Some(format!("o.object_id IN '{ids}'"))
            }
        }
        IotaObjectDataFilter::Version(v) => Some(format!("o.version = {v}")),
    }
}

fn to_outer_clauses(filter: &IotaObjectDataFilter) -> Option<String> {
    match filter {
        IotaObjectDataFilter::MatchNone(sub_filters) => {
            let sub_filters = sub_filters
                .iter()
                .flat_map(to_outer_clauses)
                .collect::<Vec<_>>();
            if sub_filters.is_empty() {
                None
            } else {
                Some(format!("NOT ({})", sub_filters.join(" OR ")))
            }
        }
        IotaObjectDataFilter::MatchAll(sub_filters) => {
            let sub_filters = sub_filters
                .iter()
                .flat_map(to_outer_clauses)
                .collect::<Vec<_>>();
            if sub_filters.is_empty() {
                None
            } else if sub_filters.len() == 1 {
                Some(sub_filters[0].to_string())
            } else {
                Some(format!("({})", sub_filters.join(" AND ")))
            }
        }
        IotaObjectDataFilter::MatchAny(sub_filters) => {
            let sub_filters = sub_filters
                .iter()
                .flat_map(to_outer_clauses)
                .collect::<Vec<_>>();
            if sub_filters.is_empty() {
                None
            } else if sub_filters.len() == 1 {
                Some(sub_filters[0].to_string())
            } else {
                Some(format!("({})", sub_filters.join(" OR ")))
            }
        }
        IotaObjectDataFilter::AddressOwner(a) => Some(format!("t1.owner_address = '{a}'")),
        _ => None,
    }
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use move_core_types::ident_str;
    use iota_json_rpc_types::IotaObjectDataFilter;
    use iota_types::{
        base_types::{ObjectID, IotaAddress},
        parse_iota_struct_tag,
    };

    use crate::store::query::DBFilter;

    #[test]
    fn test_address_filter() {
        let address = IotaAddress::from_str(
            "0x92dd4d9b0150c251661d821583ef078024ae9e9ee11063e216500861eec7f381",
        )
        .unwrap();
        let filter = IotaObjectDataFilter::AddressOwner(address);

        let expected_sql =  "SELECT t1.*
FROM (SELECT DISTINCT ON (o.object_id) *
      FROM objects_history o
      WHERE o.checkpoint <= $1
      AND ((o.owner_type = 'address_owner' AND o.owner_address = '0x92dd4d9b0150c251661d821583ef078024ae9e9ee11063e216500861eec7f381') OR (o.old_owner_type = 'address_owner' AND o.old_owner_address = '0x92dd4d9b0150c251661d821583ef078024ae9e9ee11063e216500861eec7f381'))
      ORDER BY o.object_id, version, o.checkpoint DESC) AS t1
WHERE t1.object_status NOT IN ('deleted', 'wrapped', 'unwrapped_then_deleted')
AND t1.owner_address = '0x92dd4d9b0150c251661d821583ef078024ae9e9ee11063e216500861eec7f381'
LIMIT 100;";
        assert_eq!(
            expected_sql,
            filter.to_objects_history_sql(None, 100, vec!["*"])
        );
    }

    #[test]
    fn test_move_module_filter() {
        let filter = IotaObjectDataFilter::MoveModule {
            package: ObjectID::from_str(
                "0x485d947e293f07e659127dc5196146b49cdf2efbe4b233f4d293fc56aff2aa17",
            )
            .unwrap(),
            module: ident_str!("test_module").into(),
        };
        let expected_sql = "SELECT t1.*
FROM (SELECT DISTINCT ON (o.object_id) *
      FROM objects_history o
      WHERE o.checkpoint <= $1
      AND o.object_type LIKE '0x485d947e293f07e659127dc5196146b49cdf2efbe4b233f4d293fc56aff2aa17::test_module::%'
      ORDER BY o.object_id, version, o.checkpoint DESC) AS t1
WHERE t1.object_status NOT IN ('deleted', 'wrapped', 'unwrapped_then_deleted')
LIMIT 100;";
        assert_eq!(
            expected_sql,
            filter.to_objects_history_sql(None, 100, vec!["*"])
        );
    }

    #[test]
    fn test_empty_all_filter() {
        let filter = IotaObjectDataFilter::MatchAll(vec![]);
        let expected_sql = "SELECT t1.*
FROM (SELECT DISTINCT ON (o.object_id) *
      FROM objects_history o
      WHERE o.checkpoint <= $1
      ORDER BY o.object_id, version, o.checkpoint DESC) AS t1
WHERE t1.object_status NOT IN ('deleted', 'wrapped', 'unwrapped_then_deleted')
LIMIT 100;";
        assert_eq!(
            expected_sql,
            filter.to_objects_history_sql(None, 100, vec!["*"])
        );
    }

    #[test]
    fn test_empty_any_filter() {
        let filter = IotaObjectDataFilter::MatchAny(vec![]);
        let expected_sql = "SELECT t1.*
FROM (SELECT DISTINCT ON (o.object_id) *
      FROM objects_history o
      WHERE o.checkpoint <= $1
      AND FALSE
      ORDER BY o.object_id, version, o.checkpoint DESC) AS t1
WHERE t1.object_status NOT IN ('deleted', 'wrapped', 'unwrapped_then_deleted')
LIMIT 100;";
        assert_eq!(
            expected_sql,
            filter.to_objects_history_sql(None, 100, vec!["*"])
        );
    }

    #[test]
    fn test_all_filter() {
        let filter = IotaObjectDataFilter::MatchAll(vec![
            IotaObjectDataFilter::ObjectId(
                ObjectID::from_str(
                    "0xef9fb75a7b3d4cb5551ef0b08c83528b94d5f5cd8be28b1d08a87dbbf3731738",
                )
                .unwrap(),
            ),
            IotaObjectDataFilter::StructType(parse_iota_struct_tag("0x2::test::Test").unwrap()),
        ]);

        let expected_sql = "SELECT t1.*
FROM (SELECT DISTINCT ON (o.object_id) *
      FROM objects_history o
      WHERE o.checkpoint <= $1
      AND (o.object_id = '0xef9fb75a7b3d4cb5551ef0b08c83528b94d5f5cd8be28b1d08a87dbbf3731738' AND o.object_type LIKE '0x2::test::Test%')
      ORDER BY o.object_id, version, o.checkpoint DESC) AS t1
WHERE t1.object_status NOT IN ('deleted', 'wrapped', 'unwrapped_then_deleted')
LIMIT 100;";
        assert_eq!(
            expected_sql,
            filter.to_objects_history_sql(None, 100, vec!["*"])
        );
    }
}

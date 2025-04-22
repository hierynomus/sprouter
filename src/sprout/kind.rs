use k8s_openapi::api::core::v1::{ConfigMap, Secret};
use std::{collections::BTreeMap, fmt};

use crate::utils::hash_seed_data;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SproutKind {
    ConfigMap,
    Secret,
}

pub trait AsSproutKind {
    fn sprout_kind() -> SproutKind;
    fn hash(&self) -> Option<String>;
}

impl AsSproutKind for ConfigMap {
    fn sprout_kind() -> SproutKind {
        SproutKind::ConfigMap
    }

    fn hash(&self) -> Option<String> {
        let mut merged: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        self.data.as_ref().map(|data| {
            for (k, v) in data {
                merged.insert(k.clone(), v.as_bytes().to_vec());
            }
        });
        self.binary_data.as_ref().map(|data| {
            for (k, v) in data {
                merged.insert(k.clone(), v.0.clone());
            }
        });

        if merged.is_empty() {
            None
        } else {
            Some(hash_seed_data(&merged))
        }
    }
}

impl AsSproutKind for Secret {
    fn sprout_kind() -> SproutKind {
        SproutKind::Secret
    }

    fn hash(&self) -> Option<String> {
        self.data.as_ref().map(|data| {
            let converted: BTreeMap<String, Vec<u8>> =
                data.iter().map(|(k, v)| (k.clone(), v.0.clone())).collect();
            hash_seed_data(&converted)
        })
    }
}

pub fn infer_kind<T: AsSproutKind>() -> SproutKind {
    T::sprout_kind()
}

pub fn kind_of<T: AsSproutKind>(_val: &T) -> SproutKind {
    T::sprout_kind()
}

impl fmt::Display for SproutKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind_str = match self {
            SproutKind::ConfigMap => "ConfigMap",
            SproutKind::Secret => "Secret",
        };
        write!(f, "{}", kind_str)
    }
}

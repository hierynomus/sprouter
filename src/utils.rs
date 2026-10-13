// Copyright 2025, Jeroen van Erp <jeroen@geeko.me>
// SPDX-License-Identifier: Apache-2.0
use std::collections::BTreeMap;

use kube::api::ObjectMeta;
use kube::api::ResourceExt;
use sha2::Digest;
use sha2::Sha256;

pub const LABEL_ENABLED_KEY: &str = "sprouter.geeko.me/enabled";
const SPROUT_OF_KEY: &str = "sprouter.geeko.me/sprout-of";
const SEED_HASH_KEY: &str = "sprouter.geeko.me/seed-hash";
pub const FINALIZER_KEY: &str = "sprouter.geeko.me/finalizer";

pub fn is_seed(meta: &ObjectMeta) -> bool {
    meta.labels
        .as_ref()
        .and_then(|l| l.get(LABEL_ENABLED_KEY))
        .map(|v| v == "true")
        .unwrap_or(false)
}

pub fn is_sprout(meta: &ObjectMeta) -> bool {
    meta.labels
        .as_ref()
        .and_then(|l| l.get(SPROUT_OF_KEY))
        .is_some()
}

pub fn is_sprout_recent(meta: &ObjectMeta, hash: &Option<String>) -> bool {
    meta.annotations
        .as_ref()
        .and_then(|annotations| annotations.get(SEED_HASH_KEY))
        .map(|v| v == hash.as_deref().unwrap_or_default())
        .unwrap_or(false)
}

pub fn has_finalizer(meta: &ObjectMeta) -> bool {
    meta.finalizers
        .as_ref()
        .map(|f| f.iter().any(|s| s == FINALIZER_KEY))
        .unwrap_or(false)
}

pub fn is_being_deleted(meta: &ObjectMeta) -> bool {
    meta.deletion_timestamp.is_some()
}

pub fn create_sprout<K>(r: K, hash: &Option<String>) -> K
where
    K: kube::Resource<Scope = kube::core::NamespaceResourceScope> + Clone,
{
    let mut res = r.clone();
    let val = format!("{}/{}", r.namespace().unwrap_or_default(), r.name_any());
    res.meta_mut().finalizers = None;
    res.labels_mut().remove(LABEL_ENABLED_KEY);
    res.labels_mut().remove(SEED_HASH_KEY);
    res.labels_mut()
        .insert(SPROUT_OF_KEY.to_string(), "true".to_string());
    res.annotations_mut().insert(SPROUT_OF_KEY.to_string(), val);
    if let Some(hash) = hash {
        res.annotations_mut()
            .insert(SEED_HASH_KEY.to_string(), hash.clone());
    } else {
        res.annotations_mut().remove(SEED_HASH_KEY);
    }
    res
}

pub fn hash_seed_data<V: AsRef<[u8]>>(data: &BTreeMap<String, V>) -> String {
    let mut hasher = Sha256::new();
    for (k, v) in data.iter() {
        hasher.update(k.as_bytes());
        hasher.update(v.as_ref());
    }
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::{ConfigMap, Secret};
    use kube::Resource;
    use std::collections::BTreeMap;

    #[test]
    fn test_is_seed_true() {
        let mut cm = ConfigMap::default();
        cm.metadata.labels = Some(BTreeMap::from([(
            LABEL_ENABLED_KEY.to_string(),
            "true".to_string(),
        )]));
        assert!(is_seed(cm.meta()));
    }

    #[test]
    fn test_is_seed_false_missing_label() {
        let cm = ConfigMap::default();
        assert!(!is_seed(cm.meta()));
    }

    #[test]
    fn test_is_seed_false_wrong_value() {
        let mut cm = ConfigMap::default();
        cm.metadata.labels = Some(BTreeMap::from([(
            LABEL_ENABLED_KEY.to_string(),
            "false".to_string(),
        )]));
        assert!(!is_seed(cm.meta()));
    }

    #[test]
    fn test_is_sprout_true() {
        let mut cm = ConfigMap::default();
        cm.metadata.labels = Some(BTreeMap::from([(
            SPROUT_OF_KEY.to_string(),
            "true".to_string(),
        )]));
        assert!(is_sprout(cm.meta()));
    }

    #[test]
    fn test_is_sprout_false() {
        let cm = ConfigMap::default();
        assert!(!is_sprout(cm.meta()));
    }

    #[test]
    fn test_is_sprout_recent_true() {
        let mut cm = ConfigMap::default();
        cm.metadata.annotations = Some(BTreeMap::from([(
            SEED_HASH_KEY.to_string(),
            "abc123".to_string(),
        )]));
        assert!(is_sprout_recent(cm.meta(), &Some("abc123".to_string())));
    }

    #[test]
    fn test_is_sprout_recent_false_different_hash() {
        let mut cm = ConfigMap::default();
        cm.metadata.annotations = Some(BTreeMap::from([(
            SEED_HASH_KEY.to_string(),
            "abc123".to_string(),
        )]));
        assert!(!is_sprout_recent(cm.meta(), &Some("xyz456".to_string())));
    }

    #[test]
    fn test_is_sprout_recent_false_no_label() {
        let cm = ConfigMap::default();
        assert!(!is_sprout_recent(cm.meta(), &None));
    }

    #[test]
    fn test_has_finalizer_true() {
        let mut cm = ConfigMap::default();
        cm.metadata.finalizers = Some(vec![FINALIZER_KEY.to_string()]);
        assert!(has_finalizer(cm.meta()));
    }

    #[test]
    fn test_has_finalizer_false_empty() {
        let cm = ConfigMap::default();
        assert!(!has_finalizer(cm.meta()));
    }

    #[test]
    fn test_has_finalizer_false_other_finalizer() {
        let mut cm = ConfigMap::default();
        cm.metadata.finalizers = Some(vec!["other.io/finalizer".to_string()]);
        assert!(!has_finalizer(cm.meta()));
    }

    #[test]
    fn sprouts_do_not_inherit_seed_finalizers() {
        let mut secret = Secret::default();
        secret.metadata.name = Some("seed".to_string());
        secret.metadata.namespace = Some("source".to_string());
        secret.metadata.finalizers = Some(vec![
            FINALIZER_KEY.to_string(),
            "other.example/finalizer".to_string(),
        ]);
        secret.metadata.labels = Some(BTreeMap::from([(
            LABEL_ENABLED_KEY.to_string(),
            "true".to_string(),
        )]));

        let sprout = create_sprout(secret.clone(), &None);
        assert!(sprout.metadata.finalizers.is_none());
        assert!(is_sprout(sprout.meta()));
        assert!(!is_seed(sprout.meta()));
        assert!(has_finalizer(secret.meta()));
    }

    #[test]
    fn sprout_metadata_uses_valid_labels_and_preserves_source_and_hash() {
        let mut secret = Secret::default();
        secret.metadata.name = Some("foo".to_string());
        secret.metadata.namespace = Some("default".to_string());
        secret.metadata.labels = Some(BTreeMap::from([(
            LABEL_ENABLED_KEY.to_string(),
            "true".to_string(),
        )]));
        let hash = "a".repeat(64);

        let sprout = create_sprout(secret, &Some(hash.clone()));
        let labels = sprout.metadata.labels.as_ref().unwrap();
        let annotations = sprout.metadata.annotations.as_ref().unwrap();
        assert_eq!(labels.get(SPROUT_OF_KEY).map(String::as_str), Some("true"));
        assert!(!labels.contains_key(LABEL_ENABLED_KEY));
        assert!(!labels.contains_key(SEED_HASH_KEY));
        assert_eq!(
            annotations.get(SPROUT_OF_KEY).map(String::as_str),
            Some("default/foo")
        );
        assert_eq!(annotations.get(SEED_HASH_KEY), Some(&hash));
        assert!(is_sprout_recent(sprout.meta(), &Some(hash)));
    }

    #[test]
    fn sprout_without_data_does_not_inherit_seed_hash_annotation() {
        let mut cm = ConfigMap::default();
        cm.metadata.labels = Some(BTreeMap::from([(
            SEED_HASH_KEY.to_string(),
            "a".repeat(64),
        )]));
        cm.metadata.annotations = Some(BTreeMap::from([(
            SEED_HASH_KEY.to_string(),
            "stale-hash".to_string(),
        )]));

        let sprout = create_sprout(cm, &None);
        assert!(!sprout
            .metadata
            .labels
            .as_ref()
            .unwrap()
            .contains_key(SEED_HASH_KEY));
        assert!(!sprout
            .metadata
            .annotations
            .as_ref()
            .unwrap()
            .contains_key(SEED_HASH_KEY));
    }

    #[test]
    fn test_is_being_deleted_true() {
        let meta: kube::api::ObjectMeta = serde_json::from_value(serde_json::json!({
            "deletionTimestamp": "2024-01-01T00:00:00Z"
        }))
        .unwrap();
        assert!(is_being_deleted(&meta));
    }

    #[test]
    fn test_is_being_deleted_false() {
        let cm = ConfigMap::default();
        assert!(!is_being_deleted(cm.meta()));
    }
}

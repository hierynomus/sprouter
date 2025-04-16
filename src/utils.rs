use std::collections::BTreeMap;

use kube::api::ObjectMeta;
use kube::api::ResourceExt;
use sha2::Digest;
use sha2::Sha256;

pub const ANNOTATION_KEY: &str = "sprouter.geeko.me/enabled";
const SPROUT_KEY: &str = "sprouter.geeko.me/sprout-of";
const SEED_HASH_KEY: &str = "sprouter.geeko.me/seed-hash";

pub fn is_seed(meta: &ObjectMeta) -> bool {
    meta.annotations
        .as_ref()
        .and_then(|a| a.get(ANNOTATION_KEY))
        .map(|v| v == "true")
        .unwrap_or(false)
}

pub fn is_sprout(meta: &ObjectMeta) -> bool {
    meta.annotations
        .as_ref()
        .and_then(|a| a.get(SPROUT_KEY))
        .is_some()
}

pub fn is_sprout_recent(meta: &ObjectMeta, hash: &Option<String>) -> bool {
    meta.annotations
        .as_ref()
        .and_then(|a| a.get(SEED_HASH_KEY))
        .map(|v| v == hash.as_deref().unwrap_or_default())
        .unwrap_or(false)
}

pub fn create_sprout<K>(r: K, hash: &Option<String>) -> K
where
    K: kube::Resource<Scope = kube::core::NamespaceResourceScope> + Clone,
{
    let mut res = r.clone();
    let val = format!("{}/{}", r.namespace().unwrap_or_default() , r.name_any());
    res.annotations_mut().remove(ANNOTATION_KEY);
    res.annotations_mut().insert(SPROUT_KEY.to_string(), val);
    hash.as_ref().map(|h| {
        res.annotations_mut().insert(SEED_HASH_KEY.to_string(), h.to_string());
    });
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
    use k8s_openapi::api::core::v1::ConfigMap;
    use kube::Resource;
    use std::collections::BTreeMap;

    #[test]
    fn test_is_seed_true() {
        let mut cm = ConfigMap::default();
        cm.metadata.annotations = Some(BTreeMap::from([
            (ANNOTATION_KEY.to_string(), "true".to_string()),
        ]));
        assert!(is_seed(cm.meta()));
    }

    #[test]
    fn test_is_seed_false_missing_annotation() {
        let cm = ConfigMap::default();
        assert!(!is_seed(cm.meta()));
    }

    #[test]
    fn test_is_seed_false_wrong_value() {
        let mut cm = ConfigMap::default();
        cm.metadata.annotations = Some(BTreeMap::from([
            (ANNOTATION_KEY.to_string(), "false".to_string()),
        ]));
        assert!(!is_seed(cm.meta()));
    }

    #[test]
    fn test_is_sprout_true() {
        let mut cm = ConfigMap::default();
        cm.metadata.annotations = Some(BTreeMap::from([
            (SPROUT_KEY.to_string(), "true".to_string()),
        ]));
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
        cm.metadata.annotations = Some(BTreeMap::from([
            (SPROUT_KEY.to_string(), "true".to_string()),
            (SEED_HASH_KEY.to_string(), "abc123".to_string()),
        ]));
        assert!(is_sprout_recent(cm.meta(), &Some("abc123".to_string())));
    }

    #[test]
    fn test_is_sprout_recent_false_different_hash() {
        let mut cm = ConfigMap::default();
        cm.metadata.annotations = Some(BTreeMap::from([
            (SPROUT_KEY.to_string(), "true".to_string()),
            (SEED_HASH_KEY.to_string(), "abc123".to_string()),
        ]));
        assert!(!is_sprout_recent(cm.meta(), &Some("xyz456".to_string())));
    }

    #[test]
    fn test_is_sprout_recent_false_no_annotation() {
        let cm = ConfigMap::default();
        assert!(!is_sprout_recent(cm.meta(), &None));
    }
}

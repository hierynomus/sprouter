use kube::api::ObjectMeta;
use kube::api::ResourceExt;

pub const ANNOTATION_KEY: &str = "shadower.geeko.me/enabled";
pub const SHADOW_KEY: &str = "shadower.geeko.me/shadow-of";

pub fn shadow_enabled(meta: &ObjectMeta) -> bool {
    meta.annotations
        .as_ref()
        .and_then(|a| a.get(ANNOTATION_KEY))
        .map(|v| v == "true")
        .unwrap_or(false)
}

pub fn is_shadow<K>(r: K) -> bool
where
    K: kube::Resource<Scope = kube::core::NamespaceResourceScope>,
{
    return r.annotations().contains_key(SHADOW_KEY);
}

pub fn create_shadow<K>(r: K) -> K
where
    K: kube::Resource<Scope = kube::core::NamespaceResourceScope> + Clone,
{
    let mut res = r.clone();
    let val = format!("{}/{}", r.namespace().unwrap_or_default() , r.name_any());
    res.annotations_mut().remove(ANNOTATION_KEY);
    res.annotations_mut().insert(SHADOW_KEY.to_string(), val);
    res
}

use k8s_openapi::api::core::v1::{ConfigMap, Secret};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShadowKind {
    ConfigMap,
    Secret,
}

pub trait AsShadowKind {
    fn shadow_kind() -> ShadowKind;
}

impl AsShadowKind for ConfigMap {
    fn shadow_kind() -> ShadowKind {
        ShadowKind::ConfigMap
    }
}

impl AsShadowKind for Secret {
    fn shadow_kind() -> ShadowKind {
        ShadowKind::Secret
    }
}

pub fn infer_kind<T: AsShadowKind>() -> ShadowKind {
    T::shadow_kind()
}

pub fn kind_of<T: AsShadowKind>(_val: &T) -> ShadowKind {
    T::shadow_kind()
}

impl fmt::Display for ShadowKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind_str = match self {
            ShadowKind::ConfigMap => "ConfigMap",
            ShadowKind::Secret => "Secret",
        };
        write!(f, "{}", kind_str)
    }
}

// Copyright 2025, Jeroen van Erp <jeroen@geeko.me>
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashSet;
use std::env;

pub const EXCLUDED_NAMESPACES_ENV: &str = "SPROUTER_EXCLUDED_NAMESPACES";

#[derive(Clone, Debug, Default)]
pub struct SprouterConfig {
    excluded_namespaces: HashSet<String>,
}

impl SprouterConfig {
    pub fn from_env() -> Self {
        Self::from_excluded_namespaces_env(env::var(EXCLUDED_NAMESPACES_ENV).unwrap_or_default())
    }

    pub fn from_excluded_namespaces_env(value: impl AsRef<str>) -> Self {
        let excluded_namespaces = value
            .as_ref()
            .split(',')
            .map(str::trim)
            .filter(|namespace| !namespace.is_empty())
            .map(ToString::to_string)
            .collect();

        Self {
            excluded_namespaces,
        }
    }

    pub fn is_namespace_excluded(&self, namespace: &str) -> bool {
        self.excluded_namespaces.contains(namespace)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_no_excluded_namespaces() {
        let config = SprouterConfig::default();

        assert!(!config.is_namespace_excluded("default"));
    }

    #[test]
    fn parses_comma_separated_excluded_namespaces() {
        let config = SprouterConfig::from_excluded_namespaces_env("kube-system,kube-public");

        assert!(config.is_namespace_excluded("kube-system"));
        assert!(config.is_namespace_excluded("kube-public"));
        assert!(!config.is_namespace_excluded("default"));
    }

    #[test]
    fn trims_whitespace_and_ignores_empty_values() {
        let config = SprouterConfig::from_excluded_namespaces_env(" kube-system, , default ");

        assert!(config.is_namespace_excluded("kube-system"));
        assert!(config.is_namespace_excluded("default"));
        assert!(!config.is_namespace_excluded(""));
    }
}

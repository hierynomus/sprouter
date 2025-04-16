# Sprouter Helm Chart

A Helm chart to deploy **Sprouter**, a Kubernetes operator that propagates annotated `ConfigMaps` and `Secrets` across all namespaces.

## 🧪 Features
- Automatically propagates resources with the annotation `sprouter.geeko.me/enabled: true`
- Adds a hash annotation to detect changes and prevent unnecessary updates
- Cleans up sprouts when the seed is deleted
- Propagates to new namespaces as they are created

---

## 🚀 Installation

```sh
helm repo add sprouter https://hierynomus.github.io/charts
helm install sprouter sprouter/sprouter
```

To install from a local directory:

```sh
helm install sprouter ./charts/sprouter
```

---

## 🔧 Configuration

| Key | Description | Default |
|-----|-------------|---------|
| `image.registry` | Container registry | `ghcr.io` |
| `image.repository` | Image repository | `YOUR_USER/sprouter` |
| `image.tag` | Image tag | `latest` |
| `image.pullPolicy` | Image pull policy | `IfNotPresent` |
| `global.pullSecrets` | ImagePullSecrets to use | `[]` |
| `global.imageRegistry` | Overrides `.image.registry` globally | `""` |
| `fullnameOverride` | Overrides the full resource name | `""` |
| `resources.requests` / `limits` | CPU & memory settings | See `values.yaml` |

---

## 📦 Example

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: shared-config
  annotations:
    sprouter.geeko.me/enabled: "true"
data:
  key: value
```

This ConfigMap will be automatically replicated to every namespace.

---

## 🔐 RBAC
This chart creates the following Kubernetes resources:
- ServiceAccount
- ClusterRole with scoped permissions
- ClusterRoleBinding

---

## 🧹 Uninstall

```sh
helm uninstall sprouter
```

---

## 👷 Maintainers
- @hierynomus

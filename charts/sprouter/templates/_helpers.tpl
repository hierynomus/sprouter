{{- define "sprouter.labels" -}}
app.kubernetes.io/name: {{ include "sprouter.fullname" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion | default .Chart.Version }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{- define "sprouter.fullname" }}
{{- if .Values.fullnameOverride -}}
{{ .Values.fullnameOverride | trim -}}
{{- else -}}
{{- if .Values.nameOverride -}}
{{ .Values.nameOverride | trim -}}
{{- else -}}
{{ .Release.Name | trim -}}
{{- end -}}
{{- end -}}
{{- end -}}

{{- define "sprouter.serviceAccountName" -}}
{{- if .Values.serviceAccount.enabled -}}
{{- include "sprouter.fullname" . }}
{{- else -}}
{{ .Values.serviceAccount.name | trim }}
{{- end -}}
{{- end -}}

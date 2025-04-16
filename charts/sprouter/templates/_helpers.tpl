{{- define "sprouter.labels" -}}
app.kubernetes.io/name: {{ include "sprouter.fullname" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion | default .Chart.Version }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{- define "sprouter.fullname" -}}
{{- if .Values.fullnameOverride }}
{{ .Values.fullnameOverride | trim | quote }}
{{- else }}
{{- if .Values.nameOverride }}
{{ .Values.nameOverride | trim | quote }}
{{- else }}
{{ .Release.Name | trim | quote }}
{{- end }}
{{- end }}
{{- end }}

{{- define "sprouter.serviceAccountName" -}}
{{- if .Values.serviceAccount.enabled -}}
{{ include "sprouter.fullname" . }}-sa
{{- else -}}
{{ .Values.serviceAccount.name | trim | quote }}
{{- end -}}
{{- end -}}

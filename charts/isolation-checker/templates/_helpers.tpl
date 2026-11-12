{{- define "isolation-checker.fullname" -}}
{{- if .Values.fullnameOverride -}}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" -}}
{{- else if contains .Chart.Name .Release.Name -}}
{{- .Release.Name | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- printf "%s-%s" .Release.Name .Chart.Name | trunc 63 | trimSuffix "-" -}}
{{- end -}}
{{- end -}}

{{- define "isolation-checker.selectorLabels" -}}
app.kubernetes.io/name: {{ .Chart.Name }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}

{{- define "isolation-checker.labels" -}}
{{ include "isolation-checker.selectorLabels" . }}
app.kubernetes.io/version: {{ .Values.image.tag | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version }}
{{- end -}}

{{- define "isolation-checker.serviceAccountName" -}}
{{- if .Values.serviceAccount.create -}}
{{- default (include "isolation-checker.fullname" .) .Values.serviceAccount.name -}}
{{- else -}}
{{- default "default" .Values.serviceAccount.name -}}
{{- end -}}
{{- end -}}

{{/* The file the control plane reads: only the keys it knows. */}}
{{- define "isolation-checker.platform" -}}
{{- $tenants := required "tenants is required" .Values.tenants -}}
{{- $_ := required "tenants.selector is required: the label selector that matches tenant namespaces" $tenants.selector -}}
{{- $_ := required "tenants.slis is required: requests, errors and p95Seconds queries" $tenants.slis -}}
{{- toYaml (dict "tenants" $tenants "integrations" (.Values.integrations | default dict) "chaos" .Values.chaos) -}}
{{- end -}}

{{/* Chaos Mesh resources the catalog uses, lower-cased the way the control plane names them. */}}
{{- define "isolation-checker.chaosResources" -}}
{{- $kinds := list -}}
{{- range $name, $experiment := .Values.chaos.experiments -}}
{{- with $experiment.chaos -}}
{{- $kinds = append $kinds (lower .kind) -}}
{{- end -}}
{{- end -}}
{{- $kinds | uniq | sortAlpha | toJson -}}
{{- end -}}

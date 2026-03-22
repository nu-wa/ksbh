{{/*
Expand the name of the chart.
*/}}
{{- define "ksbh.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
We truncate at 63 chars because some Kubernetes name fields are limited to this (by the DNS naming spec).
If release name contains chart name it will be used as a full name.
*/}}
{{- define "ksbh.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "ksbh.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "ksbh.labels" -}}
helm.sh/chart: {{ include "ksbh.chart" . }}
{{ include "ksbh.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "ksbh.selectorLabels" -}}
app.kubernetes.io/name: {{ include "ksbh.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create the name of the service account to use
*/}}
{{- define "ksbh.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "ksbh.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Resolve the Secret name that stores the cookie key.
*/}}
{{- define "ksbh.cookieKeySecretName" -}}
{{- default (printf "%s-cookie-key" (include "ksbh.fullname" .)) .Values.cookieKeySecret.name }}
{{- end }}

{{/*
Resolve the PVC name for writable app data.
*/}}
{{- define "ksbh.persistenceClaimName" -}}
{{- default (printf "%s-data" (include "ksbh.fullname" .)) .Values.persistence.claimName }}
{{- end }}

{{/*
Resolve the internal listener port from the configured listen address.
*/}}
{{- define "ksbh.internalPort" -}}
{{- splitList ":" .Values.app.listenAddresses.internal | last -}}
{{- end }}

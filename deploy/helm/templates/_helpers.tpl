{{/*
Expand the name of the chart.
*/}}
{{- define "opensnow.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "opensnow.fullname" -}}
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
Create chart label.
*/}}
{{- define "opensnow.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels applied to all resources.
*/}}
{{- define "opensnow.labels" -}}
helm.sh/chart: {{ include "opensnow.chart" . }}
{{ include "opensnow.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels (used in Deployment matchLabels and Service selectors).
*/}}
{{- define "opensnow.selectorLabels" -}}
app.kubernetes.io/name: {{ include "opensnow.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Resolve the container image tag: use .Values.image.tag if set, otherwise
fall back to the chart's appVersion.
*/}}
{{- define "opensnow.imageTag" -}}
{{- .Values.image.tag | default .Chart.AppVersion }}
{{- end }}

{{/*
Full image reference for the Cloud Services node.
*/}}
{{- define "opensnow.serverImage" -}}
{{- printf "%s/%s:%s" .Values.image.registry .Values.cloudServices.image.repository (include "opensnow.imageTag" .) }}
{{- end }}

{{/*
Full image reference for the Warehouse Worker.
*/}}
{{- define "opensnow.workerImage" -}}
{{- printf "%s/%s:%s" .Values.image.registry .Values.warehouseWorker.image.repository (include "opensnow.imageTag" .) }}
{{- end }}

{{/*
ServiceAccount name.
*/}}
{{- define "opensnow.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "opensnow.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
PostgreSQL host — internal service if enabled, external host otherwise.
*/}}
{{- define "opensnow.postgresHost" -}}
{{- if .Values.postgres.enabled }}
{{- printf "%s-postgres" (include "opensnow.fullname" .) }}
{{- else }}
{{- .Values.postgres.externalHost }}
{{- end }}
{{- end }}

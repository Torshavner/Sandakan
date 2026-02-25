# Infrastructure as Code: Terraform for Azure

## Status: BACKLOG

## Requirement Definition

As a **platform engineer**, I need **Terraform modules that provision all required Azure resources for the Sandakan production environment** so that **the full stack can be reproduced, torn down, and promoted across environments (dev/staging/prod) from a single `terraform apply`, with no manual Azure portal clicks**.

Additionally, every provisioned resource must **whitelist a developer's local machine IP** so that the Rust binary can be run locally (via `appsettings.Local.json`) and point directly at Azure-hosted services — no VPN, no bastion required.

---

## Context

The application already has production-ready adapters for four Azure services (OpenAI, Blob Storage, Document Intelligence, Whisper) and a `docker-compose.yml` for the local observability stack. However, there is no IaC — no Terraform, ARM templates, or CI/CD pipelines. Provisioning is currently manual.

This story creates a `terraform/` directory with **fully modular, opt-in** modules. Each module can be enabled or disabled independently via boolean `enable_*` variables, allowing a developer to provision only the Azure cognitive and storage services while keeping PostgreSQL, Qdrant, and the application container running locally. Secrets are managed via Azure Key Vault and injected at runtime via environment variables, keeping the Terraform state free of plaintext credentials.

### Plug-and-Play Principle

The primary day-to-day use case during development is:

> **Heavy compute on Azure, everything else local.**

This means `enable_openai`, `enable_document_intelligence`, and `enable_storage` default to `true`, while `enable_postgresql`, `enable_qdrant`, and `enable_container_apps` default to `false`. With this split, a developer runs the Sandakan binary locally against a local PG and Qdrant instance, but offloads LLM inference, embeddings, VLM PDF extraction, audio transcription, and file staging to Azure.

---

## Azure Services to Provision

| Service | Purpose | Terraform Resource | Default |
|---|---|---|---|
| **Resource Group** | Scope for all resources | `azurerm_resource_group` | always |
| **Azure Container Apps Environment** | Managed serverless hosting for the Rust binary | `azurerm_container_app_environment` | `false` |
| **Azure Container App** | The Sandakan application itself | `azurerm_container_app` | `false` |
| **Azure Container Registry** | Docker image registry for the app image | `azurerm_container_registry` | `false` |
| **Azure Database for PostgreSQL Flexible Server** | Relational DB (conversations, jobs, eval events, outbox) | `azurerm_postgresql_flexible_server` + `azurerm_postgresql_flexible_server_database` | `false` |
| **Azure Blob Storage Account + Container** | Staging store for uploaded files | `azurerm_storage_account` + `azurerm_storage_container` | `true` |
| **Azure OpenAI Service** | GPT-4o (chat/completions) + text-embedding-3-small | `azurerm_cognitive_account` (kind = "OpenAI") + `azurerm_cognitive_deployment` × 2 | `true` |
| **Azure AI Document Intelligence** | PDF text extraction | `azurerm_cognitive_account` (kind = "FormRecognizer") | `true` |
| **Azure Key Vault** | Secret storage for all API keys and connection strings | `azurerm_key_vault` + `azurerm_key_vault_secret` × N | `true` |
| **Azure Log Analytics Workspace** | Backend for Container Apps logs | `azurerm_log_analytics_workspace` | `false` |
| **Virtual Network + Subnet** | Network isolation for Container App Environment | `azurerm_virtual_network` + `azurerm_subnet` | `false` |

**Note on Qdrant:** When `enable_qdrant = true`, Qdrant is deployed as a Container App (using the official `qdrant/qdrant` Docker image) within the same Container Apps Environment, accessed via internal DNS. This avoids the need for a managed Qdrant cloud subscription. Default is `false` — local Qdrant via Docker is the development default.

**Note on observability stack (Loki/Tempo/Grafana):** The docker-compose observability stack is a local development tool only. In production, Azure Monitor + Application Insights is the natural equivalent — or Grafana Cloud can be connected to Azure Log Analytics. This is deferred to a future story; Container Apps logs flow to Log Analytics Workspace automatically.

---

## Directory Structure

```
terraform/
├── main.tf                    # Root module — provider + backend config, module calls with enable_* flags
├── variables.tf               # All input variables including enable_* and developer_ip
├── outputs.tf                 # Conditional outputs — null when a module is disabled
├── terraform.tfvars.example   # Two presets: "local-dev against Azure" and "full prod"
├── modules/
│   ├── resource_group/
│   │   ├── main.tf
│   │   ├── variables.tf
│   │   └── outputs.tf
│   ├── networking/
│   │   ├── main.tf            # VNet + subnet — created only when enable_container_apps = true
│   │   ├── variables.tf
│   │   └── outputs.tf
│   ├── container_registry/
│   │   ├── main.tf
│   │   ├── variables.tf
│   │   └── outputs.tf
│   ├── postgresql/
│   │   ├── main.tf            # Flexible Server + database + developer_ip firewall rule
│   │   ├── variables.tf
│   │   └── outputs.tf
│   ├── storage/
│   │   ├── main.tf            # Storage account + container + developer_ip IP rule
│   │   ├── variables.tf
│   │   └── outputs.tf
│   ├── openai/
│   │   ├── main.tf            # Cognitive account + gpt-4o + text-embedding-3-small + developer_ip network rule
│   │   ├── variables.tf
│   │   └── outputs.tf
│   ├── document_intelligence/
│   │   ├── main.tf            # Cognitive account + developer_ip network rule
│   │   ├── variables.tf
│   │   └── outputs.tf
│   ├── key_vault/
│   │   ├── main.tf            # Key Vault + secrets + developer_ip ACL
│   │   ├── variables.tf
│   │   └── outputs.tf
│   └── container_apps/
│       ├── main.tf            # Container App Environment + Sandakan app + Qdrant app
│       ├── variables.tf
│       └── outputs.tf
```

---

## Implementation Notes

### Provider & Backend

```hcl
# terraform/main.tf
terraform {
  required_providers {
    azurerm = {
      source  = "hashicorp/azurerm"
      version = "~> 4.0"
    }
  }
  backend "azurerm" {
    resource_group_name  = "sandakan-tfstate-rg"
    storage_account_name = var.tfstate_storage_account
    container_name       = "tfstate"
    key                  = "sandakan.terraform.tfstate"
  }
}
```

### Workspace Strategy

Three workspaces map to environments:

```bash
terraform workspace new dev
terraform workspace new staging
terraform workspace new prod
```

Variables scoped by workspace:

```hcl
locals {
  env = terraform.workspace  # "dev" | "staging" | "prod"
  sku_map = {
    dev     = "B_Standard_B1ms"   # PostgreSQL burstable
    staging = "GP_Standard_D2s_v3"
    prod    = "GP_Standard_D4s_v3"
  }
}
```

### Key Variables

```hcl
# terraform/variables.tf

# ── Identity & placement ──────────────────────────────────────────────────────
variable "location"      { default = "West Europe" }
variable "project_name"  { default = "sandakan" }

variable "developer_ip" {
  description = "Developer's local machine public IP for firewall whitelisting (CIDR, e.g. 1.2.3.4/32). Run: curl -s ifconfig.me"
  type        = string
}

# ── Module toggles ────────────────────────────────────────────────────────────
variable "enable_openai"                { default = true  }
variable "enable_document_intelligence" { default = true  }
variable "enable_storage"               { default = true  }
variable "enable_key_vault"             { default = true  }
variable "enable_postgresql"            { default = false }
variable "enable_qdrant"                { default = false }
variable "enable_container_apps"        { default = false }
variable "enable_container_registry"    { default = false }
variable "enable_networking"            { default = false }
variable "enable_log_analytics"         { default = false }

# ── Application ───────────────────────────────────────────────────────────────
variable "container_image" {
  description = "Docker image for the Sandakan app (e.g. acrname.azurecr.io/sandakan:latest)"
  default     = ""
}
variable "openai_gpt_capacity"   { default = 10 }  # TPM × 1000
variable "openai_embed_capacity" { default = 20 }
variable "postgres_admin_password" {
  sensitive = true
  default   = ""
}
variable "postgres_sku"     { default = "B_Standard_B1ms" }
variable "app_min_replicas" { default = 0 }
variable "app_max_replicas" { default = 3 }
```

### Module `count` Toggle Pattern

Every module receives an `enabled` variable and gates all resources behind `count`:

```hcl
# modules/postgresql/variables.tf
variable "enabled"      { default = false }
variable "developer_ip" { type = string }
# ... other vars

# modules/postgresql/main.tf
resource "azurerm_postgresql_flexible_server" "main" {
  count = var.enabled ? 1 : 0
  # ... resource config
}

resource "azurerm_postgresql_flexible_server_firewall_rule" "developer" {
  count            = var.enabled ? 1 : 0
  name             = "developer-local"
  server_id        = azurerm_postgresql_flexible_server.main[0].id
  start_ip_address = trimsuffix(var.developer_ip, "/32")
  end_ip_address   = trimsuffix(var.developer_ip, "/32")
}

# modules/postgresql/outputs.tf
output "fqdn" {
  value = var.enabled ? azurerm_postgresql_flexible_server.main[0].fqdn : null
}
```

Root module passes the toggle through:

```hcl
# terraform/main.tf
module "postgresql" {
  source       = "./modules/postgresql"
  enabled      = var.enable_postgresql
  developer_ip = var.developer_ip
  location     = var.location
  project_name = var.project_name
  # ...
}
```

### Developer IP Whitelisting — Per Service

Every service that supports network-level access control receives a firewall/IP rule scoped to `developer_ip`. The table below maps each service to its Terraform mechanism:

| Module | Terraform mechanism | Notes |
|---|---|---|
| `postgresql` | `azurerm_postgresql_flexible_server_firewall_rule` | Single IP rule (start == end) |
| `storage` | `azurerm_storage_account_network_rules` → `ip_rules` | Default action `Deny`, developer IP in allow list |
| `openai` | `azurerm_cognitive_account` → `network_acls.ip_rules` | Default action `Deny` |
| `document_intelligence` | `azurerm_cognitive_account` → `network_acls.ip_rules` | Same pattern as OpenAI |
| `key_vault` | `azurerm_key_vault` → `network_acls.ip_rules` | Default action `Deny` |
| `container_apps` (Qdrant) | Container App `ingress.ip_security_restrictions` | External ingress restricted to developer IP |

Example for Storage:

```hcl
resource "azurerm_storage_account_network_rules" "main" {
  count              = var.enabled ? 1 : 0
  storage_account_id = azurerm_storage_account.main[0].id
  default_action     = "Deny"
  ip_rules           = [trimsuffix(var.developer_ip, "/32")]
  bypass             = ["AzureServices"]
}
```

Example for OpenAI / Document Intelligence:

```hcl
resource "azurerm_cognitive_account" "main" {
  count = var.enabled ? 1 : 0
  # ...
  network_acls {
    default_action = "Deny"
    ip_rules       = [trimsuffix(var.developer_ip, "/32")]
  }
}
```

### `terraform.tfvars.example` — Two Presets

```hcl
# ─── Preset A: Local-dev binary, heavy compute on Azure ──────────────────────
# Run: cp terraform.tfvars.example dev-local.tfvars
# Edit developer_ip, then: terraform apply -var-file=dev-local.tfvars

developer_ip = "YOUR_PUBLIC_IP/32"   # curl -s ifconfig.me

enable_openai                = true
enable_document_intelligence = true
enable_storage               = true
enable_key_vault             = true
enable_postgresql            = false   # local Docker PG
enable_qdrant                = false   # local Docker Qdrant
enable_container_apps        = false   # run Sandakan binary locally
enable_container_registry    = false
enable_networking            = false
enable_log_analytics         = false


# ─── Preset B: Full production deployment ─────────────────────────────────────
# developer_ip = ""   # leave empty or set to ops IP; Container App is the ingress

# enable_openai                = true
# enable_document_intelligence = true
# enable_storage               = true
# enable_key_vault             = true
# enable_postgresql            = true
# enable_qdrant                = true
# enable_container_apps        = true
# enable_container_registry    = true
# enable_networking            = true
# enable_log_analytics         = true
```

After applying Preset A, update `appsettings.Local.json` with the Terraform outputs:

```json
{
  "llm": {
    "provider": "azure",
    "azure_endpoint": "<output: openai_endpoint>",
    "api_key": "<from Key Vault or env var>",
    "chat_model": "gpt-4o"
  },
  "embeddings": {
    "provider": "openai",
    "model": "text-embedding-3-small"
  },
  "extraction": {
    "pdf": {
      "provider": "azure",
      "azure_endpoint": "<output: document_intelligence_endpoint>"
    },
    "audio": {
      "provider": "azure",
      "azure_endpoint": "<output: openai_endpoint>",
      "azure_deployment": "whisper"
    }
  },
  "storage": {
    "provider": "azure",
    "azure_account": "<output: storage_account_name>"
  }
}
```

### Container App — Environment Variable Injection

The Container App's environment variables pull from Key Vault references via Container Apps' managed identity, keeping secrets out of Terraform state:

```hcl
resource "azurerm_container_app" "sandakan" {
  # ...
  template {
    container {
      env {
        name        = "APP_LLM_API_KEY"
        secret_name = "openai-api-key"
      }
      env {
        name  = "APP_LLM_PROVIDER"
        value = "azure"
      }
      env {
        name        = "APP_DATABASE_URL"
        secret_name = "postgres-connection-string"
      }
      # ... all APP_* settings follow the existing appsettings.Prod.json pattern
    }
  }
}
```

### Qdrant as Container App (when `enable_qdrant = true`)

```hcl
resource "azurerm_container_app" "qdrant" {
  count = var.enable_qdrant ? 1 : 0
  name  = "${var.project_name}-qdrant-${local.env}"
  # ...
  template {
    container {
      name   = "qdrant"
      image  = "qdrant/qdrant:latest"
      cpu    = 0.5
      memory = "1Gi"
    }
    volume {
      name         = "qdrant-storage"
      storage_type = "AzureFile"
      storage_name = azurerm_storage_share.qdrant[0].name
    }
  }
  ingress {
    external_enabled = false   # Internal only — Sandakan app accesses via Container App DNS
    target_port      = 6333
  }
}
```

### Conditional Outputs

```hcl
# terraform/outputs.tf
output "sandakan_app_url" {
  value = var.enable_container_apps ? module.container_apps.fqdn : null
}
output "acr_login_server" {
  value = var.enable_container_registry ? module.container_registry.login_server : null
}
output "postgres_fqdn" {
  value = var.enable_postgresql ? module.postgresql.fqdn : null
}
output "openai_endpoint" {
  value = var.enable_openai ? module.openai.endpoint : null
}
output "document_intelligence_endpoint" {
  value = var.enable_document_intelligence ? module.document_intelligence.endpoint : null
}
output "storage_account_name" {
  value = var.enable_storage ? module.storage.account_name : null
}
output "key_vault_uri" {
  value = var.enable_key_vault ? module.key_vault.uri : null
}
```

---

## File Checklist

| File | Action |
|---|---|
| `terraform/main.tf` | Create |
| `terraform/variables.tf` | Create |
| `terraform/outputs.tf` | Create |
| `terraform/terraform.tfvars.example` | Create — two presets, no secrets |
| `terraform/modules/resource_group/` | Create (3 files) |
| `terraform/modules/networking/` | Create (3 files) — gated on `enable_networking` |
| `terraform/modules/container_registry/` | Create (3 files) — gated on `enable_container_registry` |
| `terraform/modules/postgresql/` | Create (3 files) — gated on `enable_postgresql` + developer_ip firewall rule |
| `terraform/modules/storage/` | Create (3 files) — gated on `enable_storage` + developer_ip IP rule |
| `terraform/modules/openai/` | Create (3 files) — gated on `enable_openai` + developer_ip network ACL |
| `terraform/modules/document_intelligence/` | Create (3 files) — gated on `enable_document_intelligence` + developer_ip network ACL |
| `terraform/modules/key_vault/` | Create (3 files) — gated on `enable_key_vault` + developer_ip ACL |
| `terraform/modules/container_apps/` | Create (3 files) — gated on `enable_container_apps`; Qdrant sub-resource gated on `enable_qdrant` |
| `.gitignore` | Modify — add `terraform/*.tfvars`, `terraform/.terraform/`, `terraform/terraform.tfstate*` |

---

## Acceptance Criteria

```gherkin
Scenario: Fresh environment provision from zero (full prod)
  Given valid Azure credentials and a new workspace
  When terraform apply with all enable_* = true runs
  Then all Azure resources are created with no errors
  And the Sandakan app Container App is running and healthy
  And the Qdrant Container App is reachable from Sandakan internally

Scenario: Sandakan app connects to all Azure services after provision
  Given the provisioned environment
  When a PDF is ingested via POST /api/v1/ingest
  Then the file is stored in Azure Blob Storage
  And Azure Document Intelligence extracts the text
  And chunks are stored in Qdrant
  And a query via POST /api/v1/query returns an LLM-generated answer via Azure OpenAI

Scenario: Secrets are not stored in Terraform state in plaintext
  Given terraform apply has run
  When terraform state show azurerm_container_app.sandakan is executed
  Then sensitive env values show as [sensitive] or reference Key Vault URIs

Scenario: Dev and prod workspaces provision different SKUs
  Given dev workspace uses B_Standard_B1ms PostgreSQL
  And prod workspace uses GP_Standard_D4s_v3 PostgreSQL
  When both workspaces apply the same modules
  Then each environment has the correct SKU without code duplication

Scenario: terraform destroy tears down all resources cleanly
  Given a provisioned dev environment
  When terraform destroy runs
  Then all resources are removed with no orphaned dependencies

Scenario: Module-level opt-out — only Azure cognitive services provisioned
  Given enable_postgresql=false, enable_qdrant=false, enable_container_apps=false
  And enable_openai=true, enable_document_intelligence=true, enable_storage=true
  When terraform apply runs
  Then only the cognitive and storage resources are created
  And no PostgreSQL server, Container App, or VNet is provisioned
  And outputs for disabled modules are null

Scenario: Developer IP whitelisted on all provisioned services
  Given developer_ip is set to the operator's public IP in CIDR notation
  When terraform apply runs
  Then azurerm_postgresql_flexible_server_firewall_rule allows only that IP (when postgresql enabled)
  And the Storage Account network_rules ip_rules list contains only that IP
  And the Key Vault network_acls ip_rules list contains only that IP
  And the OpenAI Cognitive account network_acls ip_rules list contains only that IP
  And the Document Intelligence account network_acls ip_rules list contains only that IP

Scenario: Local Sandakan binary reaches Azure cognitive services without VPN
  Given only cognitive and storage modules are provisioned (Preset A)
  And developer_ip is whitelisted on all three services
  And appsettings.Local.json is updated with Terraform outputs
  When APP_LLM_PROVIDER=azure and APP_EXTRACTION_PDF_PROVIDER=azure are set
  Then POST /api/v1/query returns a streamed response via Azure OpenAI
  And POST /api/v1/ingest successfully extracts a PDF via Azure Document Intelligence
  And no VPN or bastion is required for any of these calls
```

---

## Test Strategy

Terraform does not have unit tests in the traditional sense. Validation is:

1. **`terraform validate`** — syntax and schema correctness. Must pass in CI.
2. **`terraform plan`** against a real Azure subscription (dev workspace) — reviewed before merge.
3. **`terraform apply`** to dev workspace as integration test — smoke-tested with the E2E Hurl collection (`collections/`).
4. **Checkov or `tfsec`** (add as CI step) — static security analysis of Terraform plans (checks for public storage accounts, missing encryption, etc.).

No mocking frameworks — Terraform's own `plan` output is the test artifact.

---

## Dependencies

- No dependency on any application code story.
- Can be implemented in parallel with all US-024–US-031 stories.
- The `appsettings.Prod.json` and `appsettings.Local.json` environment variable names (e.g. `APP_LLM_API_KEY`, `APP_LLM_AZURE_ENDPOINT`) must match the Container App environment variable names set in `modules/container_apps/main.tf`. This is a coordination point with the settings pattern established in [src/presentation/config/settings.rs](src/presentation/config/settings.rs).
- `developer_ip` must be refreshed whenever the developer's public IP changes (dynamic IPs). A helper script `scripts/update-dev-ip.sh` that runs `terraform apply -var developer_ip=$(curl -s ifconfig.me)/32 -var-file=dev-local.tfvars` is recommended but not required by this story.

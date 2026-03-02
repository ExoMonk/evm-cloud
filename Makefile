TF ?= terraform
LOCALSTACK_COMPOSE_FILE ?= tests/localstack/docker-compose.yml
AWS_REGION ?= us-east-1
LOCALSTACK_ENDPOINT ?= http://localhost:4566

LOCAL_ENV = AWS_ACCESS_KEY_ID=test AWS_SECRET_ACCESS_KEY=test AWS_SESSION_TOKEN=test AWS_REGION=$(AWS_REGION) AWS_ENDPOINT_URL=$(LOCALSTACK_ENDPOINT)

.PHONY: fmt-check validate lint security qa plan verify up down test-k8s test-e2e-k8s docs docs-dev

# --- QA ---

fmt-check:
	$(TF) fmt -check -recursive

validate:
	$(TF) init -backend=false && $(TF) validate

lint:
	tflint --recursive

security:
	checkov -d . --framework terraform

qa: fmt-check validate lint security

# --- LocalStack lifecycle ---

up:
	docker compose -f $(LOCALSTACK_COMPOSE_FILE) up -d --wait

down:
	docker compose -f $(LOCALSTACK_COMPOSE_FILE) down

# --- Plan an example against LocalStack ---
# Usage: make plan EXAMPLE=minimal_rds
#        make plan EXAMPLE=minimal_BYO_clickhouse

EXAMPLE ?= minimal_rds
EXAMPLE_DIR = examples/$(EXAMPLE)
TFVARS = $(shell ls $(EXAMPLE_DIR)/*.tfvars 2>/dev/null | grep -v auto.tfvars | grep -v secrets)

plan:
	@test -d $(EXAMPLE_DIR) || (echo "Example '$(EXAMPLE)' not found. Available:" && ls examples/ && exit 1)
	@test -n "$(TFVARS)" || (echo "No .tfvars found in $(EXAMPLE_DIR)" && exit 1)
	docker compose -f $(LOCALSTACK_COMPOSE_FILE) up -d --wait
	cd $(EXAMPLE_DIR) && $(TF) init -backend=false && $(LOCAL_ENV) $(TF) plan -var-file=$(notdir $(TFVARS)) || true
	docker compose -f $(LOCALSTACK_COMPOSE_FILE) down

# --- Verify: QA + plan all examples ---

verify:
	@$(MAKE) qa
	@for dir in examples/*/; do \
		example=$$(basename $$dir); \
		echo "\n=== Planning $$example ==="; \
		$(MAKE) plan EXAMPLE=$$example; \
	done

# --- Kind-based K8s validation ---
# Requires: kind, kubectl, helm, terraform, docker
# Creates a throwaway kind cluster, applies EKS K8s modules, validates resources.

test-k8s:
	@bash tests/kind/run.sh

# --- E2E k3s validation ---
# Requires: E2E_KUBECONFIG pointing to a persistent k3s VPS kubeconfig.
# See tests/e2e-k3s/README.md for setup instructions.
# Connects to persistent cluster, deploys via real deployer, validates, tears down.

test-e2e-k8s:
	@bash tests/e2e-k3s/run.sh

# --- Documentation ---

docs:
	cd documentation && npm run build

docs-dev:
	cd documentation && npm run dev

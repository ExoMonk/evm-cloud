TF ?= terraform
TFVARS_LOCAL ?= tests/fixtures/localstack.tfvars
TFVARS_SMOKE ?= tests/fixtures/aws-smoke.tfvars
LOCALSTACK_COMPOSE_FILE ?= tests/localstack/docker-compose.yml
AWS_REGION ?= us-east-1
AWS_PROFILE ?= default

.PHONY: preflight init fmt-check validate lint security qa localstack-up localstack-down localstack-logs local-plan local-apply local-destroy aws-smoke-plan aws-smoke-apply aws-smoke-destroy

preflight:
	@command -v $(TF) >/dev/null || (echo "terraform not installed" && exit 1)
	@command -v tflint >/dev/null || (echo "tflint not installed" && exit 1)
	@command -v checkov >/dev/null || (echo "checkov not installed" && exit 1)
	@command -v docker >/dev/null || (echo "docker not installed" && exit 1)
	@docker compose version >/dev/null 2>&1 || (echo "docker compose not available" && exit 1)

init:
	$(TF) init -backend=false

fmt-check:
	$(TF) fmt -check -recursive

validate: init
	$(TF) validate

lint:
	tflint --recursive

security:
	checkov -d . --framework terraform

qa: fmt-check validate lint security

localstack-up:
	docker compose -f $(LOCALSTACK_COMPOSE_FILE) up -d

localstack-down:
	docker compose -f $(LOCALSTACK_COMPOSE_FILE) down

localstack-logs:
	docker compose -f $(LOCALSTACK_COMPOSE_FILE) logs

local-plan: init
	mkdir -p .terraform
	AWS_ACCESS_KEY_ID=test AWS_SECRET_ACCESS_KEY=test AWS_REGION=$(AWS_REGION) $(TF) plan -var-file=$(TFVARS_LOCAL) -out=.terraform/local.plan

local-apply: local-plan
	AWS_ACCESS_KEY_ID=test AWS_SECRET_ACCESS_KEY=test AWS_REGION=$(AWS_REGION) $(TF) apply -auto-approve .terraform/local.plan

local-destroy: init
	AWS_ACCESS_KEY_ID=test AWS_SECRET_ACCESS_KEY=test AWS_REGION=$(AWS_REGION) $(TF) destroy -auto-approve -var-file=$(TFVARS_LOCAL)

aws-smoke-plan: init
	mkdir -p .terraform
	AWS_PROFILE=$(AWS_PROFILE) AWS_REGION=$(AWS_REGION) $(TF) plan -var-file=$(TFVARS_SMOKE) -out=.terraform/aws-smoke.plan

aws-smoke-apply: aws-smoke-plan
	AWS_PROFILE=$(AWS_PROFILE) AWS_REGION=$(AWS_REGION) $(TF) apply -auto-approve .terraform/aws-smoke.plan

aws-smoke-destroy: init
	AWS_PROFILE=$(AWS_PROFILE) AWS_REGION=$(AWS_REGION) $(TF) destroy -auto-approve -var-file=$(TFVARS_SMOKE)

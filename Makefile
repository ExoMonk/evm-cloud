TF ?= terraform
TFVARS_SMOKE ?= tests/fixtures/aws-smoke.tfvars
LOCALSTACK_COMPOSE_FILE ?= tests/localstack/docker-compose.yml
AWS_REGION ?= us-east-1
AWS_PROFILE ?=
AWS_SMOKE_SKIP_CREDENTIALS_VALIDATION ?= true
EXAMPLE_MINIMAL_DIR ?= examples/minimal
EXAMPLE_MINIMAL_TFVARS ?= example.tfvars
LOCALSTACK_ENDPOINT ?= http://localhost:4566

.PHONY: preflight init fmt-check validate lint security qa localstack-up localstack-down localstack-logs local-aws local-plan local-apply local-destroy local-verify example-minimal-plan example-minimal-apply example-minimal-destroy example-minimal-verify example-minimal aws-smoke-plan aws-smoke-apply aws-smoke-destroy

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

local-aws:
	@if [ -z "$(COMMAND)" ]; then \
		echo "Usage: make local-aws COMMAND='ec2 describe-vpcs'"; \
		exit 1; \
	fi
	@command -v aws >/dev/null || (echo "aws cli not installed" && exit 1)
	AWS_ACCESS_KEY_ID=test AWS_SECRET_ACCESS_KEY=test AWS_SESSION_TOKEN=test AWS_REGION=$(AWS_REGION) aws --endpoint-url=$(LOCALSTACK_ENDPOINT) $(COMMAND)

example-minimal-plan:
	cd $(EXAMPLE_MINIMAL_DIR) && terraform init -backend=false && AWS_ACCESS_KEY_ID=test AWS_SECRET_ACCESS_KEY=test AWS_SESSION_TOKEN=test AWS_REGION=$(AWS_REGION) AWS_ENDPOINT_URL=$(LOCALSTACK_ENDPOINT) terraform plan -var-file=$(EXAMPLE_MINIMAL_TFVARS) -var="networking_enabled=true" -var="aws_skip_credentials_validation=true" -out=.terraform/localstack-minimal.plan

example-minimal-apply: example-minimal-plan
	cd $(EXAMPLE_MINIMAL_DIR) && AWS_ACCESS_KEY_ID=test AWS_SECRET_ACCESS_KEY=test AWS_SESSION_TOKEN=test AWS_REGION=$(AWS_REGION) AWS_ENDPOINT_URL=$(LOCALSTACK_ENDPOINT) terraform apply -auto-approve .terraform/localstack-minimal.plan

example-minimal-destroy:
	cd $(EXAMPLE_MINIMAL_DIR) && terraform init -backend=false && AWS_ACCESS_KEY_ID=test AWS_SECRET_ACCESS_KEY=test AWS_SESSION_TOKEN=test AWS_REGION=$(AWS_REGION) AWS_ENDPOINT_URL=$(LOCALSTACK_ENDPOINT) terraform destroy -auto-approve -var-file=$(EXAMPLE_MINIMAL_TFVARS) -var="networking_enabled=true" -var="aws_skip_credentials_validation=true"

example-minimal-verify:
	cd $(EXAMPLE_MINIMAL_DIR) && terraform state list

example-minimal: localstack-up example-minimal-apply example-minimal-verify

local-plan: example-minimal-plan

local-apply: example-minimal-apply

local-destroy: example-minimal-destroy

local-verify: example-minimal-verify

aws-smoke-plan: init
	mkdir -p .terraform
	$(if $(filter true,$(AWS_SMOKE_SKIP_CREDENTIALS_VALIDATION)),AWS_ACCESS_KEY_ID=test AWS_SECRET_ACCESS_KEY=test AWS_SESSION_TOKEN=test AWS_PROFILE=,AWS_PROFILE=$(AWS_PROFILE)) AWS_REGION=$(AWS_REGION) $(TF) plan -var-file=$(TFVARS_SMOKE) -var="aws_skip_credentials_validation=$(AWS_SMOKE_SKIP_CREDENTIALS_VALIDATION)" -out=.terraform/aws-smoke.plan

aws-smoke-apply: aws-smoke-plan
	$(if $(filter true,$(AWS_SMOKE_SKIP_CREDENTIALS_VALIDATION)),AWS_ACCESS_KEY_ID=test AWS_SECRET_ACCESS_KEY=test AWS_SESSION_TOKEN=test AWS_PROFILE=,AWS_PROFILE=$(AWS_PROFILE)) AWS_REGION=$(AWS_REGION) $(TF) apply -auto-approve .terraform/aws-smoke.plan

aws-smoke-destroy: init
	$(if $(filter true,$(AWS_SMOKE_SKIP_CREDENTIALS_VALIDATION)),AWS_ACCESS_KEY_ID=test AWS_SECRET_ACCESS_KEY=test AWS_SESSION_TOKEN=test AWS_PROFILE=,AWS_PROFILE=$(AWS_PROFILE)) AWS_REGION=$(AWS_REGION) $(TF) destroy -auto-approve -var-file=$(TFVARS_SMOKE) -var="aws_skip_credentials_validation=$(AWS_SMOKE_SKIP_CREDENTIALS_VALIDATION)"

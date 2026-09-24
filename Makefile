# Named local workflows. Run `make setup-local` once; each script then loads
# the ignored .env file automatically.

.PHONY: help setup-local quickstart check tui backend broker mcp smoke-local smoke-local-call \
	docker-setup docker-up docker-status \
	docker-down docker-reset docker-bundle openbao-smoke

help:
	@printf '%s\n' 'Arqen workflows:' '  make setup-local' '  make quickstart' '  make docker-setup' '  make docker-up' '  make docker-bundle' '  make docker-status' '  make docker-down' '  make docker-reset ARQEN_DOCKER_RESET_CONFIRM=YES' '  make openbao-smoke' '  make check' '  make tui' '  make backend' '  make backend ARQEN_BACKEND_ARGS=--usurp' '  make broker' '  make mcp' '  make smoke-local' '  make smoke-local-call'

setup-local:
	./scripts/arqen-setup-local.sh

quickstart:
	./scripts/arqen-quickstart.sh $(ARQEN_QUICKSTART_ARGS)

check:
	./scripts/arqen-check.sh

tui:
	./scripts/arqen-tui.sh

backend:
	./scripts/arqen-backend.sh $(ARQEN_BACKEND_ARGS)

broker:
	./scripts/arqen-broker.sh $(ARQEN_BROKER_ARGS)

mcp:
	./scripts/arqen-mcp.sh

smoke-local:
	./scripts/arqen-smoke-local.sh

smoke-local-call:
	./scripts/arqen-smoke-local.sh --call

docker-setup:
	./scripts/arqen-docker-setup.sh

docker-up:
	./scripts/arqen-docker-up.sh

docker-bundle:
	./scripts/arqen-docker-bundle.sh

docker-status:
	./scripts/arqen-docker-status.sh

docker-down:
	./scripts/arqen-docker-down.sh

docker-reset:
	./scripts/arqen-docker-reset.sh

openbao-smoke:
	./scripts/arqen-openbao-smoke.sh

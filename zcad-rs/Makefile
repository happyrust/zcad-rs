CARGO ?= cargo
RUNTIME_PRODUCT ?= zcad
RUNTIME_PLATFORM ?=
RUNTIME_TARGET ?= runtime/dist
RUNTIME_CLEAN ?= 1
RUNTIME_ADDITIONAL_ARGS ?=
RUNTIME_ENABLED ?= 1
TYPEEXPORT_PROCESS_FILES ?= ../environment/typeexporter/zcad.files
TYPEEXPORT_DEFINES ?=
TYPEEXPORT_AUTOGEN ?=
TYPEEXPORT_EXTRA_ARGS ?=

.PHONY: fmt lint test check ci runtime app typeexport

fmt:
	$(CARGO) fmt --all

lint:
	$(CARGO) clippy --workspace --all-targets -- -D warnings

test:
	$(CARGO) test

check:
	$(CARGO) check

runtime:
ifeq ($(RUNTIME_ENABLED),0)
	@echo "runtime-builder 已禁用，跳过该步骤"
else
	$(CARGO) run -p runtime-builder -- $(if $(filter 1 true,$(RUNTIME_CLEAN)),--clean,) --target $(RUNTIME_TARGET) --product $(RUNTIME_PRODUCT) $(if $(strip $(RUNTIME_PLATFORM)),--platform $(RUNTIME_PLATFORM),) $(RUNTIME_ADDITIONAL_ARGS)
endif

ci: runtime fmt lint test

APP_ARGS ?=
app: runtime
	$(CARGO) run -p zcad-app -- $(APP_ARGS)

typeexport:
	$(CARGO) run -p runtime-builder -- typeexport --path-prefix ../cad_source $(if $(TYPEEXPORT_AUTOGEN),--autogen-dir $(TYPEEXPORT_AUTOGEN),) $(foreach f,$(TYPEEXPORT_PROCESS_FILES),--process-file $(f)) $(foreach d,$(TYPEEXPORT_DEFINES),--define $(d)) $(TYPEEXPORT_EXTRA_ARGS)

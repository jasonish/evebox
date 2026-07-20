# EveBox Makefile
#
# Requirements:
#    - GNU Make

CARGO ?= cargo

WEBAPP_SRCS := $(shell find webapp -type f | grep -v node_modules)

.PHONY: all clean evebox webapp fmt fixup

all: evebox

clean:
	rm -rf dist target resources/public resources/webapp
	find . -name \*~ -exec rm -f {} \;
	$(MAKE) -C webapp clean

resources/webapp/index.html: $(WEBAPP_SRCS)
	cd webapp && $(MAKE)
	touch src/resource.rs
webapp: resources/webapp/index.html

# Build EveBox for the host platform.
evebox: webapp
	$(CARGO) build

fmt:
	cargo fmt
	cd webapp && npm run fmt

fixup:
	$(MAKE) fmt
	cargo clippy --fix --allow-dirty

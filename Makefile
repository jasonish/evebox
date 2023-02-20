# EveBox Makefile
#
# Requirements:
#    - GNU Make

# Version info.
CARGO_VERSION	:=	$(shell cat Cargo.toml | \
			    awk '/^version/ { gsub(/"/, "", $$3); print $$3 }')
VERSION	:=		$(shell echo $(CARGO_VERSION) | \
				sed 's/\(.*\)\-.*/\1/')
VERSION_SUFFIX	:=	$(shell echo $(CARGO_VERSION) | \
				sed -n 's/.*-\(.*\)/\1/p')

BUILD_REV	?=	$(shell git rev-parse --short HEAD)
BUILD_DATE	?=	$(shell date +%s)
export BUILD_DATE

CARGO ?=	cargo

APP :=		evebox

WEBAPP_SRCS :=	$(shell find webapp -type f | grep -v node_modules)

HOST_TARGET := $(shell rustc -Vv| awk '/^host/ { print $$2 }')
TARGET ?= $(HOST_TARGET)
OS := $(shell rustc --target $(TARGET) --print cfg | awk -F'"' '/target_os/ { print $$2 }')
ifeq ($(OS),windows)
APP_EXT := .exe
endif

CARGO_BUILD_ARGS :=
ifdef TARGET
CARGO_BUILD_ARGS += --target $(TARGET)
endif

ifneq ($(VERSION_SUFFIX),)
DIST_VERSION := latest
else
DIST_VERSION :=	$(VERSION)
endif
DIST_ARCH :=	$(shell rustc --target $(TARGET) --print cfg | \
			awk -F'"' '/target_arch/ { print $$2 }' | \
			sed -e 's/x86_64/x64/' | sed -e 's/aarch64/arm64/')
EVEBOX_BIN :=	target/$(TARGET)/release/$(APP)$(APP_EXT)

all: evebox

clean:
	rm -rf dist target resources/public resource/webapp
	find . -name \*~ -exec rm -f {} \;
	$(MAKE) -C webapp clean

.PHONY: dist rpm deb

resources/webapp/index.html: $(WEBAPP_SRCS)
	cd webapp && $(MAKE)
webapp: resources/webapp/index.html

# Build's EveBox for the host platform.
evebox: webapp
	$(CARGO) build

dist: DIST_NAME ?= $(APP)-$(DIST_VERSION)-$(OS)-$(DIST_ARCH)
dist: DIST_DIR ?= dist/$(DIST_NAME)
dist: webapp
	echo "Building $(DIST_NAME)..."
	$(CARGO) build --release $(CARGO_BUILD_ARGS)
	mkdir -p $(DIST_DIR)
	cp $(EVEBOX_BIN) $(DIST_DIR)/
	mkdir -p $(DIST_DIR)/examples
	cp examples/agent.yaml $(DIST_DIR)/examples/
	cp examples/evebox.yaml $(DIST_DIR)/examples/
	cd dist && zip -r $(DIST_NAME).zip $(DIST_NAME)

fmt:
	cargo fmt
	cd webapp && npm run fmt

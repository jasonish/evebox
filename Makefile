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
DIST_NAME ?=	$(APP)-$(DIST_VERSION)-$(OS)-$(DIST_ARCH)
EVEBOX_BIN :=	target/$(TARGET)/release/$(APP)$(APP_EXT)

all: public evebox

clean:
	rm -rf dist target resources/public
	find . -name \*~ -exec rm -f {} \;
	$(MAKE) -C webapp clean

.PHONY: dist rpm deb

resources/public/_done: $(WEBAPP_SRCS)
	cd webapp && $(MAKE)
	touch $@
public: resources/public/_done

# Build's EveBox for the host platform.
evebox:
	$(CARGO) build $(RELEASE) $(CARGO_BUILD_ARGS)

dist: DIST_DIR ?= dist/$(DIST_NAME)
dist: public
	echo "Building $(DIST_NAME)..."
	$(CARGO) build --release $(CARGO_BUILD_ARGS)
	mkdir -p $(DIST_DIR)
	cp $(EVEBOX_BIN) $(DIST_DIR)/
	mkdir -p $(DIST_DIR)/examples
	cp examples/agent.yaml $(DIST_DIR)/examples/
	cp examples/evebox.yaml $(DIST_DIR)/examples/
	cd dist && zip -r $(DIST_NAME).zip $(DIST_NAME)

# Debian packaging. Due to a versioning screwup early on, we now need
# to set the epoch to 1 for those updating with apt.
deb: EPOCH := 1
ifneq ($(VERSION_SUFFIX),)
deb: TILDE := ~$(VERSION_SUFFIX)$(BUILD_DATE)
deb: OUTPUT := dist/evebox-latest-amd64.deb
else
deb: OUTPUT := dist/
endif
deb: STAGE_DIR := dist/_stage-deb
deb:
	rm -rf $(STAGE_DIR) && mkdir -p $(STAGE_DIR)
	install -m 0644 \
		examples/evebox.yaml \
		examples/agent.yaml \
		deb/evebox.default \
		deb/evebox.service \
		deb/evebox-agent.service \
		$(STAGE_DIR)
	install -m 0755 $(EVEBOX_BIN) $(STAGE_DIR)
	fpm --force -s dir \
		-t deb \
		-p $(OUTPUT) \
		-n evebox \
		--epoch $(EPOCH) \
		-v $(VERSION)$(TILDE) \
		--after-install=deb/after-install.sh \
		--after-upgrade=deb/after-upgrade.sh \
		--deb-no-default-config-files \
		--config-files /etc/default/evebox \
		$(STAGE_DIR)/evebox=/usr/bin/evebox \
	        $(STAGE_DIR)/evebox.yaml=/etc/evebox/evebox.yaml.example \
		$(STAGE_DIR)/agent.yaml=/etc/evebox/agent.yaml.example \
		$(STAGE_DIR)/evebox.default=/etc/default/evebox \
		$(STAGE_DIR)/evebox.service=/lib/systemd/system/evebox.service \
		$(STAGE_DIR)/evebox-agent.service=/lib/systemd/system/evebox-agent.service
	rm -rf $(STAGE_DIR)
	ar p dist/*.deb data.tar.gz | tar ztvf -

# RPM packaging.
ifneq ($(VERSION_SUFFIX),)
# Setup non-release versioning.
rpm: RPM_ITERATION := 0.$(VERSION_SUFFIX)$(BUILD_DATE)
rpm: OUTPUT := dist/evebox-latest-x86_64.rpm
else
# Setup release versioning.
rpm: RPM_ITERATION := 1
rpm: OUTPUT := dist/
endif
rpm:
	fpm --force -s dir \
	    -t rpm \
	    -p $(OUTPUT) \
	    -n evebox \
	    -v $(VERSION) \
	    --iteration $(RPM_ITERATION) \
	    --before-install=rpm/before-install.sh \
	    --after-upgrade=rpm/after-upgrade.sh \
	    --config-files /etc/sysconfig/evebox \
	    --config-files /etc/evebox/exebox.yaml \
	    --config-files /etc/evebox/agen.yaml \
	    --rpm-attr 0644,root,root:/lib/systemd/system/evebox.service \
	    --rpm-attr 0644,root,root:/lib/systemd/system/evebox-agent.service \
	    --rpm-attr 0755,root,root:/usr/bin/evebox \
	    --rpm-attr 0640,root,evebox:/etc/evebox/evebox.yaml \
	    --rpm-attr 0640,root,evebox:/etc/evebox/agent.yaml \
	    --rpm-attr 0644,root,root:/etc/sysconfig/evebox \
	    ${EVEBOX_BIN}=/usr/bin/evebox \
	    examples/evebox.yaml=/etc/evebox/evebox.yaml \
	    examples/agent.yaml=/etc/evebox/agent.yaml \
	    rpm/evebox.sysconfig=/etc/sysconfig/evebox \
	    rpm/evebox.service=/lib/systemd/system/evebox.service \
	    rpm/evebox-agent.service=/lib/systemd/system/evebox-agent.service

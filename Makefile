# EveBox Makefile
#
# Requirements:
#    - GNU Make

# Version info.
VERSION_SUFFIX	:=	dev
VERSION		:=	0.12.0
BUILD_REV	?=	$(shell git rev-parse --short HEAD)
BUILD_DATE	?=	$(shell date +%s)
export BUILD_DATE

APP :=		evebox

WEBAPP_SRCS :=	$(shell find webapp -type f | grep -v node_modules)

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
	cargo build $(RELEASE) --target $(TARGET) --features "$(FEATURES)"

release: RELEASE := "--release"
release: evebox

HOST_TARGET := $(shell rustc -Vv| awk '/^host/ { print $$2 }')
TARGET ?= $(HOST_TARGET)
OS := $(shell rustc --target $(TARGET) --print cfg | awk -F'"' '/target_os/ { print $$2 }')
ifeq ($(OS),windows)
APP_EXT := .exe
endif

ifneq ($(VERSION_SUFFIX),)
dist: VERSION := latest
endif
dist: DISTARCH := $(shell rustc --target $(TARGET) --print cfg | awk -F'"' '/target_arch/ { print $$2 }' | \
	sed -e 's/x86_64/x64/')
dist: DISTNAME ?= $(APP)-$(VERSION)-$(OS)-$(DISTARCH)
dist: DISTDIR ?= dist/$(DISTNAME)
dist: public
	cargo build --release --target $(TARGET) --features "$(FEATURES)"
	mkdir -p $(DISTDIR)
	cp target/$(TARGET)/release/$(APP)$(APP_EXT) $(DISTDIR)/
	cp agent.yaml.example $(DISTDIR)/
	cp evebox.yaml.example $(DISTDIR)/
	cd dist && zip -r $(DISTNAME).zip $(DISTNAME)

# Debian packaging. Due to a versioning screwup early on, we now need
# to set the epoch to 1 for those updating with apt.
deb: EPOCH := 1
ifneq ($(VERSION_SUFFIX),)
deb: TILDE := ~$(VERSION_SUFFIX)$(BUILD_DATE)
deb: EVEBOX_BIN := dist/$(APP)-latest-linux-x64/evebox
deb: OUTPUT := dist/evebox-latest-amd64.deb
else
deb: EVEBOX_BIN := dist/$(APP)-$(VERSION)-linux-x64/evebox
deb: OUTPUT := dist/
endif
deb: STAGE := dist/_stage-deb
deb:
	rm -rf $(STAGE)
	mkdir -p $(STAGE)
	install -m 0644 \
		evebox.yaml.example \
		agent.yaml.example \
		deb/evebox.default \
		deb/evebox.service \
		deb/evebox-agent.service \
		$(STAGE)
	install -m 0755 $(EVEBOX_BIN) $(STAGE)
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
		$(STAGE)/evebox=/usr/bin/evebox \
	        $(STAGE)/evebox.yaml.example=/etc/evebox/evebox.yaml.example \
		$(STAGE)/agent.yaml.example=/etc/evebox/agent.yaml.example \
		$(STAGE)/evebox.default=/etc/default/evebox \
		$(STAGE)/evebox.service=/lib/systemd/system/evebox.service \
		$(STAGE)/evebox-agent.service=/lib/systemd/system/evebox-agent.service
	ar p dist/*.deb data.tar.gz | tar ztvf -

# RPM packaging.
ifneq ($(VERSION_SUFFIX),)
# Setup non-release versioning.
rpm: RPM_ITERATION := 0.$(VERSION_SUFFIX)$(BUILD_DATE)
rpm: EVEBOX_BIN := dist/$(APP)-latest-linux-x64/evebox
rpm: OUTPUT := dist/evebox-latest-x86_64.rpm
else
# Setup release versioning.
rpm: RPM_ITERATION := 1
rpm: EVEBOX_BIN := dist/$(APP)-$(VERSION)-linux-x64/evebox
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
	    --rpm-attr 0644,root,root:/lib/systemd/system/evebox.service \
	    --rpm-attr 0644,root,root:/lib/systemd/system/evebox-agent.service \
	    --rpm-attr 0755,root,root:/usr/bin/evebox \
	    --rpm-attr 0644,root,root:/etc/evebox/evebox.yaml.example \
	    --rpm-attr 0644,root,root:/etc/evebox/agent.yaml.example \
	    --rpm-attr 0644,root,root:/etc/sysconfig/evebox.service \
	    ${EVEBOX_BIN}=/usr/bin/evebox \
	    evebox.yaml.example=/etc/evebox/evebox.yaml.example \
	    agent.yaml.example=/etc/evebox/agent.yaml.example \
	    rpm/evebox.sysconfig=/etc/sysconfig/evebox \
	    rpm/evebox.service=/lib/systemd/system/evebox.service \
	    rpm/evebox-agent.service=/lib/systemd/system/evebox-agent.service

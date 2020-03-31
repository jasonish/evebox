# EveBox Makefile
#
# Requirements:
#    - GNU Make

# Version info.
#VERSION_SUFFIX	:=	dev
VERSION		:=	0.11.1
BUILD_REV	:=	$(shell git rev-parse --short HEAD)
BUILD_DATE	?=	$(shell git log --pretty=format:%ct -1)
export BUILD_DATE

BUILD_REV_VAR :=	github.com/jasonish/evebox/core.BuildRev
BUILD_VER_VAR :=	github.com/jasonish/evebox/core.BuildVersion
LDFLAGS :=	-X \"$(BUILD_REV_VAR)=$(BUILD_REV)\" \
		-X \"$(BUILD_VER_VAR)=$(VERSION)$(VERSION_SUFFIX)\" \

HOST_GOOS :=	$(shell go env GOOS)
HOST_GOARCH :=	$(shell go env GOARCH)
HOST_DIST :=	$(HOST_GOOS)/$(HOST_GOARCH)

GOOS ?=		$(shell go env GOOS)
GOARCH ?=	$(shell go env GOARCH)
DIST :=		$(GOOS)/$(GOARCH)

ifeq ($(HOST_DIST),$(DIST))
CGO_ENABLED :=	1
TAGS +=		fts5 json1
endif

APP :=		evebox

GO_SRCS :=	$(shell find . -name \*.go | grep -v /vendor/)
GO_PACKAGES =	$(shell go list ./... | grep -v /vendor/)

WEBAPP_SRCS :=	$(shell find webapp -type f | grep -v node_modules)

GOPATH ?=	$(HOME)/go

all: public evebox

install-deps:
	go get github.com/gobuffalo/packr/packr
	cd webapp && $(MAKE) install-deps

update-deps:
	go get -u github.com/cespare/reflex
	go get -u github.com/gobuffalo/packr/packr

clean:
	rm -rf dist
	rm -f evebox
	rm -rf resources/public
	$(GOPATH)/bin/packr clean
	find . -name \*~ -exec rm -f {} \;

distclean: clean
	rm -rf vendor
	$(MAKE) -C webapp $@

.PHONY: dist rpm deb

resources/public/_done: $(WEBAPP_SRCS)
	cd webapp && $(MAKE)
	touch $@
public: resources/public/_done

# Build's EveBox for the host platform.
evebox: Makefile $(GO_SRCS)
	$(GOPATH)/bin/packr -z -i resources
	CGO_ENABLED=$(CGO_ENABLED) go build ${RACE} --tags "$(TAGS)" \
		-ldflags "$(LDFLAGS)" \
		cmd/evebox.go

# Format all go source code except in the vendor directory.
gofmt:
	@go fmt $(GO_PACKAGES)

dist: GOARCH ?= $(shell go env GOARCH)
dist: GOOS ?= $(shell go env GOOS)
dist: DISTARCH := $(GOARCH)
ifeq ($(GOARCH),amd64)
dist: DISTARCH := x64
endif
ifeq ($(GOARCH),386)
dist: DISTARCH := x32
endif
ifneq ($(VERSION_SUFFIX),)
dist: VERSION := latest
endif
dist: DISTNAME ?= ${APP}$(DIST_SUFFIX)-${VERSION}-${GOOS}-${DISTARCH}
dist: LDFLAGS += -s
dist: CGO_ENABLED ?= $(CGO_ENABLED)
ifeq ($(GOOS),windows)
dist: APP_EXT := .exe
endif
dist: public
	@echo "Building EveBox rev $(BUILD_REV)."
	$(GOPATH)/bin/packr -z -i resources
	CGO_ENABLED=$(CGO_ENABLED) GOARCH=$(GOARCH) GOOS=$(GOOS) \
		go build -tags "$(TAGS)" -ldflags "$(LDFLAGS)" \
		-o dist/$(DISTNAME)/${APP}${APP_EXT} cmd/evebox.go
	cp agent.yaml.example dist/$(DISTNAME)
	cp evebox.yaml.example dist/$(DISTNAME)
	cd dist && zip -r ${DISTNAME}.zip ${DISTNAME}

# Debian packaging. Due to a versioning screwup early on, we now need
# to set the epoch to 1 for those updating with apt.
deb: EPOCH := 1
ifneq ($(VERSION_SUFFIX),)
deb: TILDE := ~$(VERSION_SUFFIX)$(BUILD_DATE)
deb: EVEBOX_BIN := dist/${APP}-latest-linux-x64/evebox
deb: OUTPUT := dist/evebox-latest-amd64.deb
else
deb: EVEBOX_BIN := dist/${APP}-${VERSION}-linux-x64/evebox
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
rpm: EVEBOX_BIN := dist/${APP}-latest-linux-x64/evebox
rpm: OUTPUT := dist/evebox-latest-x86_64.rpm
else
# Setup release versioning.
rpm: RPM_ITERATION := 1
rpm: EVEBOX_BIN := dist/${APP}-${VERSION}-linux-x64/evebox
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
